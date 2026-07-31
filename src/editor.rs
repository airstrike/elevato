//! The Rhai editor pane: a self-contained TEA cell owning the text
//! buffer, the dirty flag, and the Reset backup. The parent owns
//! everything the buttons *do* — compiling, running, persisting — and
//! is told about it via [`Instruction`]; the current script error
//! (compile or runtime) is a read-only prop passed into [`State::view`].

use iced::widget::{button, column, container, row, text, text_editor};
use iced::{Element, Fill};

use crate::action::Action;
use crate::highlight;
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
                self.dirty = self.dirty || action.is_edit();
                self.content.perform(action);
                Action::none()
            }
            Message::Apply => Action::instruction(Instruction::Apply(self.text())),
            Message::Save => Action::instruction(Instruction::Save(self.text())),
            Message::Reset => {
                self.backup = Some(self.text());
                self.content = text_editor::Content::with_text(playback::STARTER);
                self.dirty = true;
                Action::none()
            }
            Message::UndoReset => {
                if let Some(backup) = self.backup.take() {
                    self.content = text_editor::Content::with_text(&backup);
                    self.dirty = true;
                }
                Action::none()
            }
        }
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
            .highlight_with::<highlight::Highlighter>((), |kind, theme| kind.format(theme))
            .style(theme::text_editor::code);

        let buttons = row![
            button(text("Apply").size(13)).on_press(Message::Apply),
            button(text("Save").size(13)).on_press_maybe(self.dirty.then_some(Message::Save)),
            button(text("Reset").size(13)).on_press(Message::Reset),
            button(text("Undo reset").size(13))
                .on_press_maybe(self.backup.is_some().then_some(Message::UndoReset)),
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
}
