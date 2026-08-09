//! The API reference pane: `assets/api.rhai` rendered read-only in a
//! text editor with the same Rhai highlighting as the code pane, plus
//! the jump registry behind cmd+click — every definition line in the
//! document is a navigation target, from here or from the editor.

use std::collections::HashMap;
use std::sync::LazyLock;

use iced::widget::text_editor;
use iced::{Element, Fill};

use crate::action::Action;
use crate::highlight;
use crate::theme;

/// The reference document itself — the single source of truth for both
/// the rendered pane and the jump registry.
pub const SOURCE: &str = include_str!("../assets/api.rhai");

/// Name → zero-based line of its definition in [`SOURCE`]. A definition
/// is a non-comment line: leading `fn` / `let` / `event` / `type`
/// keywords and `elevator.` / `floor.` receivers are stripped, and the
/// leading identifier of what remains is the name. First sighting wins,
/// so overloads (`go_to_floor`) share one entry.
static REGISTRY: LazyLock<HashMap<String, usize>> = LazyLock::new(|| {
    let mut registry = HashMap::new();
    for (line, text) in SOURCE.lines().enumerate() {
        let Some(name) = definition_name(text) else {
            continue;
        };
        registry.entry(name.to_string()).or_insert(line);
    }
    registry
});

/// The line where `name` is defined, if the reference documents it.
pub fn lookup(name: &str) -> Option<usize> {
    REGISTRY.get(name).copied()
}

/// Extracts the defined name from one line of [`SOURCE`], per the
/// registry's convention.
fn definition_name(line: &str) -> Option<&str> {
    let mut rest = line.trim_start();
    if rest.is_empty() || rest.starts_with("//") {
        return None;
    }
    for keyword in ["fn ", "let ", "event ", "type "] {
        rest = rest.strip_prefix(keyword).unwrap_or(rest).trim_start();
    }
    for receiver in ["elevator.", "floor."] {
        rest = rest.strip_prefix(receiver).unwrap_or(rest);
    }
    let end = rest
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(rest.len());
    (end > 0).then(|| &rest[..end])
}

/// The identifier under `column` in `line`, if any. Shared by the docs
/// pane and the editor: both resolve cmd+clicks the same way.
pub fn identifier_in(line: &str, column: usize) -> Option<String> {
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let chars: Vec<char> = line.chars().collect();
    // A click lands *between* characters; probe the char at the cursor
    // and, failing that, the one before it (end-of-word clicks).
    let mut at = column.min(chars.len().saturating_sub(1));
    if chars.get(at).is_none_or(|&c| !is_word(c)) {
        at = column.checked_sub(1)?;
        if chars.get(at).is_none_or(|&c| !is_word(c)) {
            return None;
        }
    }
    let start = (0..=at).rev().take_while(|&i| is_word(chars[i])).last()?;
    let end = (at..chars.len())
        .take_while(|&i| is_word(chars[i]))
        .last()?;
    let word: String = chars[start..=end].iter().collect();
    // Identifiers don't start with a digit.
    (!word.starts_with(|c: char| c.is_ascii_digit())).then_some(word)
}

/// The docs pane's state.
pub struct State {
    content: text_editor::Content,
}

/// Docs pane messages.
#[derive(Debug, Clone)]
pub enum Message {
    /// The viewer performed an action (click, scroll, selection…).
    Viewed(text_editor::Action),
}

impl State {
    /// A viewer over [`SOURCE`], scrolled to the top.
    pub fn new() -> Self {
        Self {
            content: text_editor::Content::with_text(SOURCE),
        }
    }

    /// Applies a viewer message. Edits are silently dropped — the
    /// reference is read-only; everything else (clicks, selection,
    /// scrolling, copy) behaves like a normal editor.
    pub fn update(&mut self, message: Message) -> Action<(), Message> {
        match message {
            Message::Viewed(action) => {
                if !action.is_edit() {
                    self.content.perform(action);
                }
                Action::none()
            }
        }
    }

    /// The identifier under the cursor (set by the click just
    /// performed), for cmd+click resolution.
    pub fn identifier_at_cursor(&self) -> Option<String> {
        let position = self.content.cursor().position;
        identifier_in(&self.content.line(position.line)?.text, position.column)
    }

    /// Moves the cursor to `line` and selects it, scrolling it into
    /// view. Rebuilt from the top so repeated jumps stay cheap and
    /// deterministic.
    pub fn jump_to(&mut self, line: usize) {
        use text_editor::{Action, Motion};
        self.content.perform(Action::Move(Motion::DocumentStart));
        for _ in 0..line {
            self.content.perform(Action::Move(Motion::Down));
        }
        self.content.perform(Action::Select(Motion::End));
    }

    /// The pane: the reference under the same highlighter and style as
    /// the code editor.
    pub fn view(&self) -> Element<'_, Message> {
        text_editor(&self.content)
            .height(Fill)
            .size(13)
            .font(theme::MONO)
            .on_action(Message::Viewed)
            .highlight_with::<highlight::Highlighter>((), |kind, theme| kind.format(theme))
            .style(theme::text_editor::code)
            .into()
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The API contract: every name a script can touch. If a rename or
    /// addition in `script/` forgets the reference, this list fails.
    const CONTRACT: &[&str] = &[
        "init",
        "update",
        "dt",
        "elevators",
        "floors",
        "Elevator",
        "Floor",
        "go_to_floor",
        "stop",
        "check_destination_queue",
        "destination_queue",
        "set_destination_queue",
        "current_floor",
        "max_passenger_count",
        "load_factor",
        "is_full",
        "destination_direction",
        "pressed_floors",
        "move_count",
        "is_busy",
        "is_moving",
        "is_on_a_floor",
        "going_up_indicator",
        "going_down_indicator",
        "on",
        "floor_num",
        "level",
        "up_pressed",
        "down_pressed",
        "idle",
        "floor_button_pressed",
        "passing_floor",
        "stopped_at_floor",
        "up_button_pressed",
        "down_button_pressed",
    ];

    #[test]
    fn every_name_in_the_api_contract_has_a_documented_definition() {
        let missing: Vec<&str> = CONTRACT
            .iter()
            .copied()
            .filter(|name| lookup(name).is_none())
            .collect();
        assert!(missing.is_empty(), "undocumented API names: {missing:?}");
    }

    #[test]
    fn definitions_resolve_to_their_own_lines() {
        let line = lookup("load_factor").expect("load_factor is documented");
        let text = SOURCE.lines().nth(line).unwrap();
        assert!(text.contains("load_factor"));
        assert!(!text.trim_start().starts_with("//"));
    }

    #[test]
    fn identifiers_resolve_under_and_just_after_the_word() {
        let line = "    elevator.go_to_floor(n, force);";
        assert_eq!(identifier_in(line, 15).as_deref(), Some("go_to_floor"));
        assert_eq!(identifier_in(line, 24).as_deref(), Some("go_to_floor"));
        assert_eq!(identifier_in(line, 26).as_deref(), Some("n"));
        assert_eq!(identifier_in(line, 2), None);
    }

    #[test]
    fn event_names_inside_string_literals_resolve_like_identifiers() {
        let line = r#"    elevator.on("idle", || {});"#;
        assert_eq!(identifier_in(line, 18).as_deref(), Some("idle"));
        assert!(lookup("idle").is_some());
    }

    #[test]
    fn jumping_lands_the_cursor_on_the_requested_line() {
        let mut state = State::new();
        let line = lookup("stopped_at_floor").unwrap();
        state.jump_to(line);
        assert_eq!(state.content.cursor().position.line, line);
    }
}
