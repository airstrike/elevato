//! The Rhai editor pane: a self-contained TEA cell owning the text
//! buffer, the dirty flag, and the Reset backup. The parent owns
//! everything the buttons *do* — compiling, running, persisting — and
//! is told about it via [`Instruction`]; the current script error
//! (compile or runtime) is a read-only prop passed into [`State::view`].

use iced::widget::{button, column, container, row, space, text, text_editor};
use iced::{Element, Fill};

use crate::action::Action;
use crate::highlight;
use crate::icon;
use crate::playback;
use crate::theme;

/// The editor pane's state.
pub struct State {
    content: text_editor::Content,
    /// Whether the text has changed since the parent last confirmed a
    /// save ([`State::mark_saved`]); gates the Save button.
    dirty: bool,
    /// The text Reset replaced, kept for Undo reset. `Content` is not
    /// `Clone`, so backups travel as text snapshots.
    backup: Option<String>,
    /// Undo history: text snapshots taken *before* each edit burst
    /// (consecutive typing coalesces into one entry). Restoring puts
    /// the cursor at the buffer start — a v1 simplification.
    undo: Vec<String>,
    /// Redone-able snapshots; cleared by any fresh edit.
    redo: Vec<String>,
    /// The kind of the previous edit, for coalescing.
    last_edit: Option<Edit>,
}

/// How an edit action groups for undo purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edit {
    /// A plain character insertion — bursts coalesce.
    Typing,
    /// Anything else (newline, delete, paste) — snapshots individually.
    Other,
}

/// Undo history depth; older snapshots fall off the far end.
const UNDO_DEPTH: usize = 200;

/// Classifies an edit action for undo grouping.
fn edit_kind(action: &text_editor::Action) -> Edit {
    match action {
        text_editor::Action::Edit(text_editor::Edit::Insert(character))
            if !character.is_whitespace() =>
        {
            Edit::Typing
        }
        _ => Edit::Other,
    }
}

/// Editor pane messages.
#[derive(Debug, Clone)]
pub enum Message {
    /// The text editor performed an action (edit, cursor move, …).
    Edited(text_editor::Action),
    /// Apply was pressed: compile and run the current text.
    Apply,
    /// Save was pressed: persist the current text.
    Save,
    /// Reset was pressed: back the current text up, restore the
    /// starter program.
    Reset,
    /// Undo reset was pressed: restore the backed-up text.
    UndoReset,
    /// Cmd/Ctrl+Z.
    Undo,
    /// Cmd/Ctrl+Shift+Z.
    Redo,
}

/// What the parent must do on the editor's behalf.
pub enum Instruction {
    /// Compile and run this source on the current challenge,
    /// persisting it on success.
    Apply(String),
    /// Persist this source (whether or not it compiles).
    Save(String),
}

impl State {
    /// An editor holding `code`, clean and with nothing to undo.
    pub fn new(code: &str) -> Self {
        Self {
            content: text_editor::Content::with_text(code),
            dirty: false,
            backup: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_edit: None,
        }
    }

    /// The current source text.
    pub fn text(&self) -> String {
        self.content.text()
    }

    /// The parent confirms the current text was persisted; the Save
    /// button disables until the next edit.
    pub fn mark_saved(&mut self) {
        self.dirty = false;
    }

    /// Applies an editor message.
    pub fn update(&mut self, message: Message) -> Action<Instruction, Message> {
        match message {
            Message::Edited(action) => {
                if action.is_edit() {
                    self.record(edit_kind(&action));
                    self.dirty = true;
                }
                self.content.perform(action);
                Action::none()
            }
            Message::Apply => Action::instruction(Instruction::Apply(self.text())),
            Message::Save => Action::instruction(Instruction::Save(self.text())),
            Message::Reset => {
                self.record(Edit::Other);
                self.backup = Some(self.text());
                self.content = text_editor::Content::with_text(playback::STARTER);
                self.dirty = true;
                Action::none()
            }
            Message::UndoReset => {
                if let Some(backup) = self.backup.take() {
                    self.record(Edit::Other);
                    self.content = text_editor::Content::with_text(&backup);
                    self.dirty = true;
                }
                Action::none()
            }
            Message::Undo => {
                if let Some(snapshot) = self.undo.pop() {
                    self.redo.push(self.text());
                    self.content = text_editor::Content::with_text(&snapshot);
                    self.dirty = true;
                    self.last_edit = None;
                }
                Action::none()
            }
            Message::Redo => {
                if let Some(snapshot) = self.redo.pop() {
                    self.undo.push(self.text());
                    self.content = text_editor::Content::with_text(&snapshot);
                    self.dirty = true;
                    self.last_edit = None;
                }
                Action::none()
            }
        }
    }

    /// Books the pre-edit text into the undo history. Consecutive
    /// typing coalesces into the burst's first snapshot; anything else
    /// snapshots individually. Any fresh edit invalidates redo.
    fn record(&mut self, kind: Edit) {
        self.redo.clear();
        if kind == Edit::Other || self.last_edit != Some(Edit::Typing) {
            self.undo.push(self.text());
            if self.undo.len() > UNDO_DEPTH {
                self.undo.remove(0);
            }
        }
        self.last_edit = Some(kind);
    }

    /// The pane: editor, error panel (when `error` is present), and
    /// the Apply/Save/Reset/Undo-reset row. `error` is whichever
    /// script error is current — compile from Apply or runtime from a
    /// tick — owned by the parent.
    pub fn view<'a>(&'a self, error: Option<&'a script::Error>) -> Element<'a, Message> {
        let editor = text_editor(&self.content)
            .height(Fill)
            .size(14)
            .font(theme::MONO)
            .on_action(Message::Edited)
            .key_binding(|press| {
                let z = matches!(press.key.as_ref(), iced::keyboard::Key::Character("z"));
                if z && press.modifiers.command() {
                    Some(text_editor::Binding::Custom(if press.modifiers.shift() {
                        Message::Redo
                    } else {
                        Message::Undo
                    }))
                } else {
                    text_editor::Binding::from_key_press(press)
                }
            })
            .highlight_with::<highlight::Highlighter>((), |kind, theme| kind.format(theme))
            .style(theme::text_editor::code);

        let buttons = row![
            button(
                row![icon::check().size(13), text("Apply").size(13)]
                    .spacing(6)
                    .align_y(iced::Center)
            )
            .on_press(Message::Apply)
            .style(theme::button::primary),
            button(
                row![icon::save().size(13), text("Save").size(13)]
                    .spacing(6)
                    .align_y(iced::Center)
            )
            .on_press_maybe(self.dirty.then_some(Message::Save))
            .style(theme::button::outline),
            space::horizontal(),
            button(
                row![icon::eraser().size(13), text("Reset").size(13)]
                    .spacing(6)
                    .align_y(iced::Center)
            )
            .on_press(Message::Reset)
            .style(theme::button::ghost),
            button(
                row![icon::undo_2().size(13), text("Undo reset").size(13)]
                    .spacing(6)
                    .align_y(iced::Center)
            )
            .on_press_maybe(self.backup.is_some().then_some(Message::UndoReset))
            .style(theme::button::ghost),
        ]
        .spacing(8);

        let mut pane = column![editor].spacing(8);
        if let Some(error) = error {
            pane = pane.push(error_panel(error));
        }
        pane = pane.push(buttons);

        container(pane)
            .width(Fill)
            .height(Fill)
            .padding(8)
            .style(theme::container::panel)
            .into()
    }
}

/// The error strip under the editor. `script::Error`'s display carries
/// the rhai position (line and column) when one exists.
fn error_panel<'a>(error: &script::Error) -> Element<'a, Message> {
    container(text(error.to_string()).size(12).style(theme::text::failure))
        .width(Fill)
        .padding(8)
        .style(theme::container::error_panel)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn insert(state: &mut State, character: char) {
        let _ = state.update(Message::Edited(text_editor::Action::Edit(
            text_editor::Edit::Insert(character),
        )));
    }

    #[test]
    fn reset_backs_up_the_current_text_and_restores_the_starter() {
        let mut state = State::new("fn init(elevators, floors) {}\n");
        let _ = state.update(Message::Reset);
        assert_eq!(state.text().trim_end(), playback::STARTER.trim_end());
        assert!(state.dirty);
        assert!(state.backup.is_some());
    }

    #[test]
    fn undo_reset_restores_the_backup_exactly_once() {
        let mut state = State::new("fn init(elevators, floors) {}\n");
        let _ = state.update(Message::Reset);
        let _ = state.update(Message::UndoReset);
        assert_eq!(state.text().trim_end(), "fn init(elevators, floors) {}");
        assert!(state.backup.is_none());
        // A second undo with no backup changes nothing.
        let _ = state.update(Message::UndoReset);
        assert_eq!(state.text().trim_end(), "fn init(elevators, floors) {}");
    }

    #[test]
    fn apply_and_save_emit_the_current_source_as_instructions() {
        let mut state = State::new("fn init(e, f) {}");
        let apply = state.update(Message::Apply);
        assert!(matches!(
            apply.instruction,
            Some(Instruction::Apply(source)) if source.contains("fn init")
        ));
        let save = state.update(Message::Save);
        assert!(matches!(
            save.instruction,
            Some(Instruction::Save(source)) if source.contains("fn init")
        ));
    }

    #[test]
    fn an_edit_marks_the_editor_dirty_until_the_parent_confirms_a_save() {
        let mut state = State::new("fn init(e, f) {}");
        assert!(!state.dirty);
        insert(&mut state, 'x');
        assert!(state.dirty);
        state.mark_saved();
        assert!(!state.dirty);
        insert(&mut state, 'y');
        assert!(state.dirty);
    }

    #[test]
    fn undo_restores_the_text_before_a_typing_burst() {
        let mut state = State::new("fn init(e, f) {}");
        insert(&mut state, 'a');
        insert(&mut state, 'b');
        insert(&mut state, 'c');
        let _ = state.update(Message::Undo);
        assert_eq!(state.text().trim_end(), "fn init(e, f) {}");
    }

    #[test]
    fn redo_reapplies_an_undone_burst_and_dies_on_fresh_edits() {
        let mut state = State::new("fn init(e, f) {}");
        insert(&mut state, 'x');
        let _ = state.update(Message::Undo);
        let _ = state.update(Message::Redo);
        assert!(state.text().starts_with('x'));
        let _ = state.update(Message::Undo);
        insert(&mut state, 'y');
        let _ = state.update(Message::Redo);
        assert!(
            state.text().starts_with('y'),
            "redo must die after a fresh edit"
        );
    }

    #[test]
    fn a_reset_is_undoable_like_any_other_edit() {
        let mut state = State::new("fn init(e, f) {}");
        let _ = state.update(Message::Reset);
        let _ = state.update(Message::Undo);
        assert_eq!(state.text().trim_end(), "fn init(e, f) {}");
    }
}
