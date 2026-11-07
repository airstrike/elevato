//! The Rust editor pane: a self-contained TEA cell owning the text
//! buffer, the dirty flag, the Reset backup, and the strip that says
//! what became of the last applied edit. The parent owns the writes
//! (it is told via [`Instruction`] and answers through
//! [`State::applied`] and [`State::saved`]) and passes rawr's verdicts
//! down through [`State::reloaded`]; the program's panic, when it
//! stopped the run, is a read-only prop passed into [`State::view`].
//!
//! Apply (the button, or Cmd+Enter / Ctrl+Enter) saves the program for
//! whatever runs it: under `rawr run` the save is the reload.

use std::io;

use elevato_api::simulation;
use iced::keyboard::{self, key::Named};
use iced::widget::{button, column, container, row, space, text, text_editor};
use iced::{Element, Fill};

use crate::action::Action;
use crate::docs;
use crate::highlight;
use crate::icon;
use crate::playback;
use crate::reload;
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
    /// the cursor at the buffer start - a v1 simplification.
    undo: Vec<String>,
    /// Redone-able snapshots; cleared by any fresh edit.
    redo: Vec<String>,
    /// The kind of the previous edit, for coalescing.
    last_edit: Option<Edit>,
    /// What became of the last applied edit, shown in the strip.
    status: Option<Status>,
}

/// What the strip says about the last applied edit.
#[derive(Debug)]
enum Status {
    /// Apply wrote the file rawr watches; its verdict hasn't arrived.
    Sent,
    /// rawr's verdict on a save.
    Reloaded(reload::Outcome),
    /// Saved, but the program is compiled into this binary or page and
    /// can't change while it runs.
    Compiled,
    /// The write failed, so nothing was applied.
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
    /// Apply was pressed, or Cmd+Enter typed: save the current text and
    /// hand it to the running game.
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
    /// Save this source as the program, for the running game to take up;
    /// answer with [`State::applied`].
    Apply(String),
    /// Save this source as the program (whether or not it compiles);
    /// answer with [`State::saved`].
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
            status: None,
        }
    }

    /// The current source text.
    pub fn text(&self) -> String {
        self.content.text()
    }

    /// The parent's answer to [`Instruction::Apply`]: whether the text
    /// was written, for a program `host` runs. Under rawr the strip then
    /// waits for rawr's verdict.
    pub fn applied(&mut self, written: io::Result<()>, host: reload::Host) {
        self.status = Some(match written {
            Ok(()) => {
                self.dirty = false;
                match host {
                    reload::Host::Rawr => Status::Sent,
                    reload::Host::Compiled => Status::Compiled,
                }
            }
            Err(error) => Status::Unsaved(error),
        });
    }

    /// The parent's answer to [`Instruction::Save`]: whether the text was
    /// written. Only a failure shows; what rawr does with the save
    /// arrives on its own.
    pub fn saved(&mut self, written: io::Result<()>) {
        match written {
            Ok(()) => self.dirty = false,
            Err(error) => self.status = Some(Status::Unsaved(error)),
        }
    }

    /// rawr's verdict on a save, from this editor or any other.
    pub fn reloaded(&mut self, outcome: reload::Outcome) {
        self.status = Some(Status::Reloaded(outcome));
    }

    /// The identifier under the cursor (just set by a click), for the
    /// parent's cmd+click documentation jumps.
    pub fn identifier_at_cursor(&self) -> Option<String> {
        let position = self.content.cursor().position;
        docs::identifier_in(&self.content.line(position.line)?.text, position.column)
    }

    /// The selection, when it is exactly one identifier - the
    /// double-tap jump affordance's input.
    pub fn selected_identifier(&self) -> Option<String> {
        let selection = self.content.selection()?;
        Some(docs::identifier(&selection)?.to_string())
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
            Message::Apply => {
                if !self.dirty {
                    return Action::none();
                }
                Action::instruction(Instruction::Apply(self.text()))
            }
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

    /// The pane: editor, the status strip (once an edit was applied),
    /// the error panel (when `error` is present), and the
    /// Apply/Save/Reset/Undo-reset row. `error` is the program's panic
    /// that stopped the run, owned by the parent.
    pub fn view<'a>(&'a self, error: Option<&'a simulation::Error>) -> Element<'a, Message> {
        let editor = text_editor(&self.content)
            .height(Fill)
            .size(14)
            .font(theme::MONO)
            .on_action(Message::Edited)
            .key_binding(|press| {
                let command = press.modifiers.command();
                if command && press.key == keyboard::Key::Named(Named::Enter) {
                    return Some(text_editor::Binding::Custom(Message::Apply));
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
            .highlight_with::<highlight::Highlighter>((), |kind, theme| kind.format(theme))
            .style(theme::text_editor::code);

        let buttons = row![
            button(
                row![icon::check().size(13), text("Apply").size(13)]
                    .spacing(6)
                    .align_y(iced::Center)
            )
            .on_press_maybe(self.dirty.then_some(Message::Apply))
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

        // The parent's workspace card provides the surface, border, and
        // padding - this pane is chromeless content.
        let mut pane = column![editor].spacing(8);
        if let Some(status) = &self.status {
            pane = pane.push(strip(status));
        }
        if let Some(error) = error {
            pane = pane.push(error_panel(error));
        }
        pane.push(buttons).into()
    }
}

/// The status strip under the editor: what became of the last applied
/// edit, in one line.
fn strip<'a>(status: &Status) -> Element<'a, Message> {
    let (line, tone) = describe(status);
    let line = text(line).size(12);
    let (line, panel): (_, fn(&iced::Theme) -> container::Style) = match tone {
        Tone::Quiet => (
            line.style(theme::text::secondary),
            theme::container::status_panel,
        ),
        Tone::Good => (
            line.style(theme::text::outcome(true)),
            theme::container::status_panel,
        ),
        Tone::Bad => (
            line.style(theme::text::failure),
            theme::container::error_panel,
        ),
    };
    container(line).width(Fill).padding(8).style(panel).into()
}

/// How a strip line reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tone {
    Quiet,
    Good,
    Bad,
}

/// The strip's line for `status`, and its tone.
fn describe(status: &Status) -> (String, Tone) {
    match status {
        Status::Sent => (
            "Applying: rawr is checking your edit.".to_string(),
            Tone::Quiet,
        ),
        Status::Reloaded(reload::Outcome::Queued) => (
            "Compiled. The new code takes over at the program's next message.".to_string(),
            Tone::Quiet,
        ),
        Status::Reloaded(reload::Outcome::Swapped) => (
            "Applied. The program kept its state; the new code handles the next message."
                .to_string(),
            Tone::Good,
        ),
        Status::Reloaded(reload::Outcome::Restarting(reason)) => (
            format!("Restarting with a fresh world: {reason}."),
            Tone::Quiet,
        ),
        Status::Reloaded(reload::Outcome::Restarted(reason)) => (
            format!("Fresh attempt with your new code. Its state couldn't carry over: {reason}."),
            Tone::Quiet,
        ),
        Status::Reloaded(reload::Outcome::Failed(failure)) => (
            format!(
                "Line {}:{}: {}. The last good program keeps running.",
                failure.line(),
                failure.column(),
                failure.message().trim_end_matches('.')
            ),
            Tone::Bad,
        ),
        Status::Compiled if cfg!(target_arch = "wasm32") => (
            "Saved as a draft: the page runs the program it was built with.".to_string(),
            Tone::Quiet,
        ),
        Status::Compiled => (
            "Saved for the next build: under cargo run, the program can't change while it runs."
                .to_string(),
            Tone::Quiet,
        ),
        Status::Unsaved(error) => (format!("Not applied: saving failed ({error})."), Tone::Bad),
    }
}

/// The error strip under the editor: the panic's message.
fn error_panel<'a>(error: &simulation::Error) -> Element<'a, Message> {
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
    fn apply_and_save_emit_the_current_source_as_instructions() {
        let mut state = State::new("fn new() -> Model {}");
        insert(&mut state, 'x');
        let apply = state.update(Message::Apply);
        assert!(matches!(
            apply.instruction,
            Some(Instruction::Apply(source)) if source.contains("fn new")
        ));
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

    /// The strip's line and tone, as the player reads them.
    fn strip_line(state: &State) -> Option<(String, Tone)> {
        state.status.as_ref().map(describe)
    }

    #[test]
    fn a_clean_editor_has_nothing_to_apply() {
        let mut state = State::new("fn new() -> Model {}");
        assert!(state.update(Message::Apply).instruction.is_none());
        insert(&mut state, 'x');
        assert!(state.update(Message::Apply).instruction.is_some());
        state.applied(Ok(()), reload::Host::Rawr);
        assert!(state.update(Message::Apply).instruction.is_none());
    }

    #[test]
    fn an_edit_applied_under_rawr_waits_for_its_verdict() {
        let mut state = State::new("");
        insert(&mut state, 'x');
        state.applied(Ok(()), reload::Host::Rawr);
        let (line, tone) = strip_line(&state).unwrap();
        assert!(line.contains("rawr is checking"), "{line}");
        assert_eq!(tone, Tone::Quiet);
        assert!(!state.dirty);
    }

    #[test]
    fn a_swap_says_the_state_was_kept() {
        let mut state = State::new("");
        state.reloaded(reload::Outcome::Queued);
        assert!(strip_line(&state).unwrap().0.contains("next message"));
        state.reloaded(reload::Outcome::Swapped);
        let (line, tone) = strip_line(&state).unwrap();
        assert!(line.contains("kept its state"), "{line}");
        assert_eq!(tone, Tone::Good);
    }

    #[test]
    fn a_restart_says_why_in_one_line() {
        let reason = "`Program` gained a field".to_string();
        let mut state = State::new("");
        state.reloaded(reload::Outcome::Restarting(reason.clone()));
        assert!(strip_line(&state).unwrap().0.contains("gained a field"));
        state.reloaded(reload::Outcome::Restarted(reason));
        let (line, tone) = strip_line(&state).unwrap();
        assert!(line.starts_with("Fresh attempt"), "{line}");
        assert!(line.contains("gained a field"), "{line}");
        assert!(!line.contains('\n'));
        assert_eq!(tone, Tone::Quiet);
    }

    #[test]
    fn a_refused_save_shows_its_error_and_position() {
        let mut state = State::new("");
        state.reloaded(reload::Outcome::Failed(reload::Failure::new(
            "cannot find value `nxt` in this scope",
            42,
            17,
        )));
        let (line, tone) = strip_line(&state).unwrap();
        assert!(
            line.starts_with("Line 42:17: cannot find value `nxt`"),
            "{line}"
        );
        assert!(line.contains("last good program keeps running"), "{line}");
        assert_eq!(tone, Tone::Bad);
    }

    #[test]
    fn a_compiled_program_says_it_cannot_change() {
        let mut state = State::new("");
        insert(&mut state, 'x');
        state.applied(Ok(()), reload::Host::Compiled);
        let (line, _) = strip_line(&state).unwrap();
        assert!(line.contains("can't change while it runs"), "{line}");
    }

    #[test]
    fn a_failed_write_applies_nothing_and_keeps_the_edit_dirty() {
        let mut state = State::new("");
        insert(&mut state, 'x');
        state.applied(Err(io::Error::other("disk full")), reload::Host::Rawr);
        let (line, tone) = strip_line(&state).unwrap();
        assert!(line.contains("disk full"), "{line}");
        assert_eq!(tone, Tone::Bad);
        assert!(state.dirty);
    }
}
