//! The Rust editor owns its text, undo history, Reset backup, and error strip.
//! The parent writes saves requested through [`Instruction`] and reports them
//! through [`State::saved`] and [`State::reloaded`]. [`State::view`] borrows
//! any panic that ends the player's run.
//!
//! Save (the button, or Cmd+S) writes the program; under `rawr run` that write
//! is what applies the edit. Cmd+Enter saves too, and starts the run when it
//! isn't running.

use std::io;

use elevato_api::simulation;
use iced::keyboard::{self, key::Named};
use iced::widget::{button, column, container, row, space, text, text_editor};
use iced::{Element, Fill, Widget};

use crate::action::Action;
use crate::docs;
use crate::highlight::{self, Kind};
use crate::icon;
use crate::playback;
use crate::reload;
use crate::resolution::{self, Scope};
use crate::theme;

/// The editor pane's state.
pub struct State {
    content: text_editor::Content,
    /// Whether the text has changed since the parent last confirmed a save
    /// ([`State::saved`]); gates Save.
    dirty: bool,
    /// The text Reset replaced, kept for Undo reset. `Content` is not `Clone`,
    /// so backups travel as text snapshots.
    backup: Option<String>,
    /// Text snapshots from before each edit burst. Consecutive typing shares
    /// one entry; restoring places the cursor at the buffer start.
    undo: Vec<String>,
    /// Snapshots available for redo; cleared by any fresh edit.
    redo: Vec<String>,
    /// The kind of the previous edit, for coalescing.
    last_edit: Option<Edit>,
    /// What the strip shows, when anything went wrong.
    status: Option<Status>,
}

/// What the strip shows.
#[derive(Debug)]
enum Status {
    /// rawr refused a save.
    Failed(reload::Failure),
    /// Writing the program failed.
    Unsaved(io::Error),
}

/// How an edit action groups for undo purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edit {
    /// A plain character insertion - bursts coalesce.
    Typing,
    /// Anything else (newline, delete, paste) - snapshots individually.
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
    /// Save was pressed, or Cmd+S typed: write the current text as the program.
    Save,
    /// Cmd+Enter: save, and start the run when it isn't running.
    Run,
    /// Reset was pressed: back the current text up, restore the starter
    /// program.
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
    /// Save this source as the program (whether or not it compiles); answer
    /// with [`State::saved`].
    Save(String),
    /// Start the run when it isn't running, first saving this source when there
    /// are unsaved edits (answer with [`State::saved`]).
    Run(Option<String>),
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
            status: None,
        }
    }

    /// The current source text.
    pub fn text(&self) -> String {
        self.content.text()
    }

    /// Reports the write requested by [`Instruction::Save`]. Success clears
    /// write errors only; reload outcomes clear compiler errors. Saving
    /// unchanged text produces no reload outcome.
    pub fn saved(&mut self, written: io::Result<()>) {
        match written {
            Ok(()) => {
                self.dirty = false;
                if matches!(self.status, Some(Status::Unsaved(_))) {
                    self.status = None;
                }
            }
            Err(error) => self.status = Some(Status::Unsaved(error)),
        }
    }

    /// rawr's verdict on a save, from this editor or any other. Only a refusal
    /// shows.
    pub fn reloaded(&mut self, outcome: reload::Outcome) {
        self.status = match outcome {
            reload::Outcome::Failed(failure) => Some(Status::Failed(failure)),
            reload::Outcome::Restarted(_)
            | reload::Outcome::Queued
            | reload::Outcome::Swapped
            | reload::Outcome::Restarting(_) => None,
        };
    }

    /// Resolves the name under the cursor for the parent's cmd+click jump.
    pub fn reference_at_cursor(&self) -> Option<(docs::Page, usize)> {
        let position = self.content.cursor().position;
        resolution::resolve(
            Scope::Program(&self.content.text()),
            position.line,
            position.index,
        )
    }

    /// The selection, when it is exactly one identifier, and the library
    /// definition it names - the double-tap jump affordance's input.
    pub fn selected_reference(&self) -> Option<(String, docs::Page, usize)> {
        let name = docs::identifier(&self.content.selection()?)?;
        let (page, line) = self.reference_at_cursor()?;
        Some((name, page, line))
    }

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
            Message::Save => {
                if !self.dirty {
                    return Action::none();
                }
                Action::instruction(Instruction::Save(self.text()))
            }
            Message::Run => Action::instruction(Instruction::Run(self.dirty.then(|| self.text()))),
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

    /// Records the text before an edit. Consecutive typing shares the burst's
    /// first snapshot; other edits each get one. Any edit clears redo.
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

    /// The pane: editor, the strip (when there is something to show), the error
    /// panel (when `error` is present), and the Save/Reset/Undo-reset row.
    /// `error` is the program's panic that stopped the run, owned by the
    /// parent.
    pub fn view<'a>(&'a self, error: Option<&'a simulation::Error>) -> Element<'a, Message> {
        let editor = text_editor(&self.content)
            .height(Fill)
            .size(14)
            .font(theme::MONO)
            .on_action(Message::Edited)
            .key_binding(|press| {
                let command = press.modifiers.command();
                if command && press.key == keyboard::Key::Named(Named::Enter) {
                    return Some(text_editor::Binding::Custom(Message::Run));
                }
                if command && matches!(press.key.as_ref(), keyboard::Key::Character("s")) {
                    return Some(text_editor::Binding::Custom(Message::Save));
                }
                let z = matches!(press.key.as_ref(), keyboard::Key::Character("z"));
                if z && command {
                    Some(text_editor::Binding::Custom(if press.modifiers.shift() {
                        Message::Redo
                    } else {
                        Message::Undo
                    }))
                } else {
                    text_editor::Binding::from_key_press(press)
                }
            })
            .highlight_with::<highlight::Parser>((), Kind::style)
            .style(theme::text_editor::code);

        let buttons = row![
            button(
                row![icon::save().size(13), text("Save").size(13)]
                    .spacing(6)
                    .align_y(iced::Center)
            )
            .on_press_maybe(self.dirty.then_some(Message::Save))
            .style(theme::button::primary),
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

        // The parent's workspace card provides the surface, border, and
        // padding - this pane is chromeless content.
        let mut pane = column![editor].spacing(8);
        if let Some(status) = &self.status {
            pane = pane.push(strip(status));
        }
        if let Some(error) = error {
            pane = pane.push(error_panel(error));
        }
        pane.push(buttons.boxed()).boxed()
    }
}

/// The strip under the editor: a refused edit's error, or a failed write.
fn strip<'a>(status: &Status) -> Element<'a, Message> {
    container(text(describe(status)).size(12).style(theme::text::failure))
        .width(Fill)
        .padding(8)
        .style(theme::container::error_panel)
        .boxed()
}

/// The strip's line for `status`.
fn describe(status: &Status) -> String {
    match status {
        Status::Failed(failure) => match failure.at() {
            Some(at) => format!("Line {}:{}: {}", at.line, at.column, failure.message()),
            None => failure.message().to_string(),
        },
        Status::Unsaved(_) if cfg!(target_arch = "wasm32") => {
            "Couldn't save your code.".to_string()
        }
        Status::Unsaved(error) => format!("Couldn't save: {error}"),
    }
}

/// The error strip under the editor: the panic's message.
fn error_panel<'a>(error: &simulation::Error) -> Element<'a, Message> {
    container(text(error.to_string()).size(12).style(theme::text::failure))
        .width(Fill)
        .padding(8)
        .style(theme::container::error_panel)
        .boxed()
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
        let mut state = State::new("fn new() -> Model {}\n");
        let _ = state.update(Message::Reset);
        assert_eq!(state.text().trim_end(), playback::STARTER.trim_end());
        assert!(state.dirty);
        assert!(state.backup.is_some());
    }

    #[test]
    fn undo_reset_restores_the_backup_exactly_once() {
        let mut state = State::new("fn new() -> Model {}\n");
        let _ = state.update(Message::Reset);
        let _ = state.update(Message::UndoReset);
        assert_eq!(state.text().trim_end(), "fn new() -> Model {}");
        assert!(state.backup.is_none());
        // A second undo with no backup changes nothing.
        let _ = state.update(Message::UndoReset);
        assert_eq!(state.text().trim_end(), "fn new() -> Model {}");
    }

    #[test]
    fn save_emits_the_current_source_as_an_instruction() {
        let mut state = State::new("fn new() -> Model {}");
        insert(&mut state, 'x');
        let save = state.update(Message::Save);
        assert!(matches!(
            save.instruction,
            Some(Instruction::Save(source)) if source.contains("fn new")
        ));
    }

    #[test]
    fn an_edit_marks_the_editor_dirty_until_the_parent_confirms_a_save() {
        let mut state = State::new("fn new() -> Model {}");
        assert!(!state.dirty);
        insert(&mut state, 'x');
        assert!(state.dirty);
        state.saved(Ok(()));
        assert!(!state.dirty);
        insert(&mut state, 'y');
        assert!(state.dirty);
    }

    #[test]
    fn undo_restores_the_text_before_a_typing_burst() {
        let mut state = State::new("fn new() -> Model {}");
        insert(&mut state, 'a');
        insert(&mut state, 'b');
        insert(&mut state, 'c');
        let _ = state.update(Message::Undo);
        assert_eq!(state.text().trim_end(), "fn new() -> Model {}");
    }

    #[test]
    fn redo_reapplies_an_undone_burst_and_dies_on_fresh_edits() {
        let mut state = State::new("fn new() -> Model {}");
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
        let mut state = State::new("fn new() -> Model {}");
        let _ = state.update(Message::Reset);
        let _ = state.update(Message::Undo);
        assert_eq!(state.text().trim_end(), "fn new() -> Model {}");
    }

    /// The strip's line, as the player reads it.
    fn strip_line(state: &State) -> Option<String> {
        state.status.as_ref().map(describe)
    }

    #[test]
    fn run_saves_unsaved_edits_and_runs_either_way() {
        let mut state = State::new("fn new() -> Model {}");
        assert!(matches!(
            state.update(Message::Run).instruction,
            Some(Instruction::Run(None))
        ));
        insert(&mut state, 'x');
        assert!(matches!(
            state.update(Message::Run).instruction,
            Some(Instruction::Run(Some(source))) if source.starts_with('x')
        ));
    }

    #[test]
    fn save_needs_an_unsaved_edit() {
        let mut state = State::new("fn new() -> Model {}");
        assert!(state.update(Message::Save).instruction.is_none());
        insert(&mut state, 'x');
        assert!(state.update(Message::Save).instruction.is_some());
        state.saved(Ok(()));
        assert!(state.update(Message::Save).instruction.is_none());
    }

    #[test]
    fn a_successful_save_shows_nothing() {
        let mut state = State::new("");
        insert(&mut state, 'x');
        state.saved(Ok(()));
        assert_eq!(strip_line(&state), None);
        for outcome in [
            reload::Outcome::Restarted("`Program` changed its fields".to_string()),
            reload::Outcome::Queued,
            reload::Outcome::Swapped,
            reload::Outcome::Restarting("`Program` changed its fields".to_string()),
        ] {
            state.reloaded(outcome);
            assert_eq!(strip_line(&state), None);
        }
    }

    #[test]
    fn a_refused_save_shows_its_error_at_its_position() {
        let mut state = State::new("");
        state.reloaded(reload::Outcome::Failed(reload::Failure::new(
            "cannot find value `nxt` in this scope",
            Some(reload::Position {
                line: 42,
                column: 17,
            }),
        )));
        assert_eq!(
            strip_line(&state).as_deref(),
            Some("Line 42:17: cannot find value `nxt` in this scope")
        );
        insert(&mut state, 'x');
        state.saved(Ok(()));
        assert!(strip_line(&state).is_some(), "a save alone keeps the error");
        state.reloaded(reload::Outcome::Swapped);
        assert_eq!(strip_line(&state), None);
    }

    #[test]
    fn a_failed_write_shows_the_error_and_keeps_the_edit_dirty() {
        let mut state = State::new("");
        insert(&mut state, 'x');
        state.saved(Err(io::Error::other("disk full")));
        assert_eq!(
            strip_line(&state).as_deref(),
            Some("Couldn't save: disk full")
        );
        assert!(state.dirty);
    }
}
