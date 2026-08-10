//! The API reference: three Rust-voiced pages (`lib.rs`,
//! `elevator.rs`, `floor.rs`) rendered read-only through the same
//! highlighter as the code pane, plus the jump registry behind
//! cmd+click — struct fields, methods, enum variants (and their
//! snake_case event names), and entry points all navigate, from the
//! reference or from the editor.

use std::collections::HashMap;
use std::sync::LazyLock;

use iced::widget::{column, container, text, text_editor};
use iced::{Element, Fill};

use crate::action::Action;
use crate::highlight;
use crate::theme;

/// One page of the reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    /// Program shape: `new`, `update`, the commands, gotchas,
    /// determinism.
    Lib,
    /// The `Elevator` snapshot struct and its events.
    Elevator,
    /// The `Floor` snapshot struct and its events.
    Floor,
}

impl Page {
    /// Every page, in resolution-preference order for editor clicks.
    pub const ALL: &[Self] = &[Self::Elevator, Self::Floor, Self::Lib];

    /// The page's source text.
    pub fn source(self) -> &'static str {
        match self {
            Self::Lib => include_str!("../assets/api/lib.rs"),
            Self::Elevator => include_str!("../assets/api/elevator.rs"),
            Self::Floor => include_str!("../assets/api/floor.rs"),
        }
    }

    /// The file name shown above the pane.
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Lib => "lib.rs",
            Self::Elevator => "elevator.rs",
            Self::Floor => "floor.rs",
        }
    }
}

/// Name → every definition site carrying it. `Event` and `on` exist on
/// two pages; [`resolve`] picks by preference.
static REGISTRY: LazyLock<HashMap<String, Vec<(Page, usize)>>> = LazyLock::new(|| {
    let mut registry: HashMap<String, Vec<(Page, usize)>> = HashMap::new();
    let mut insert = |name: String, page, line| {
        let sites = registry.entry(name).or_default();
        // First sighting per page wins (overloads share an entry).
        if !sites.iter().any(|&(p, _)| p == page) {
            sites.push((page, line));
        }
    };
    for &page in Page::ALL {
        for (line, text) in page.source().lines().enumerate() {
            let Some(name) = definition_name(text) else {
                continue;
            };
            insert(name.to_string(), page, line);
            // Enum variants are CamelCase; scripts name the same event
            // in snake_case inside `on("…")` — register both.
            if name.starts_with(|c: char| c.is_ascii_uppercase()) {
                insert(snake_case(name), page, line);
            }
        }
    }
    // Entry-point parameters read like globals in scripts; point them
    // at their declaring signature — `update` receives all three
    // (`new` has no parameters).
    for (alias, target) in [
        ("message", "update"),
        ("dt", "update"),
        ("elevators", "update"),
        ("floors", "update"),
    ] {
        if let Some(&(page, line)) = registry.get(target).and_then(|sites| sites.first()) {
            registry
                .entry(alias.to_string())
                .or_default()
                .push((page, line));
        }
    }
    registry
});

/// The definition site for `name`, preferring `preferred`'s page (so a
/// click on `Event` inside `floor.rs` stays on `floor.rs`), then the
/// [`Page::ALL`] order.
pub fn resolve(name: &str, preferred: Option<Page>) -> Option<(Page, usize)> {
    let sites = REGISTRY.get(name)?;
    preferred
        .and_then(|page| sites.iter().find(|&&(p, _)| p == page))
        .or_else(|| sites.first())
        .copied()
}

/// Extracts the defined name from one line of a page: `pub struct X`,
/// `pub enum X`, `pub x: T`, `pub fn x(…)`, `fn x(…)`, or a bare
/// CamelCase enum-variant line.
fn definition_name(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return None;
    }
    let mut rest = trimmed;
    for keyword in ["pub ", "struct ", "enum ", "fn "] {
        rest = rest.strip_prefix(keyword).unwrap_or(rest);
    }
    let end = rest
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(rest.len());
    if end == 0 {
        return None;
    }
    let name = &rest[..end];
    if rest == trimmed {
        // No keyword was stripped: only bare CamelCase variant lines
        // count as definitions (`Idle,`, `PassingFloor { … }`).
        return name
            .starts_with(|c: char| c.is_ascii_uppercase())
            .then_some(name);
    }
    Some(name)
}

/// `FloorButtonPressed` → `floor_button_pressed`.
fn snake_case(name: &str) -> String {
    let mut snake = String::with_capacity(name.len() + 4);
    for (index, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if index > 0 {
                snake.push('_');
            }
            snake.push(c.to_ascii_lowercase());
        } else {
            snake.push(c);
        }
    }
    snake
}

/// `text` when it is exactly one identifier — what a double click (or
/// double tap) leaves selected on a name.
pub fn identifier(text: &str) -> Option<&str> {
    let word = text.trim();
    (!word.is_empty()
        && !word.starts_with(|c: char| c.is_ascii_digit())
        && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
    .then_some(word)
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
    page: Page,
    content: text_editor::Content,
    /// Where cmd+clicks came from, newest last — the back stack.
    history: Vec<(Page, usize)>,
    /// Where back() came from, newest last — the forward stack.
    future: Vec<(Page, usize)>,
}

/// How deep the back stack grows before old entries fall off.
const HISTORY_DEPTH: usize = 32;

/// Docs pane messages.
#[derive(Debug, Clone)]
pub enum Message {
    /// The viewer performed an action (click, scroll, selection…).
    Viewed(text_editor::Action),
}

impl State {
    /// A viewer opened on `lib.rs`, at the top.
    pub fn new() -> Self {
        Self {
            page: Page::Lib,
            content: text_editor::Content::with_text(Page::Lib.source()),
            history: Vec::new(),
            future: Vec::new(),
        }
    }

    /// The page currently shown.
    pub fn page(&self) -> Page {
        self.page
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

    /// The selection, when it is exactly one identifier — the
    /// double-tap jump affordance's input.
    pub fn selected_identifier(&self) -> Option<String> {
        let selection = self.content.selection()?;
        Some(identifier(&selection)?.to_string())
    }

    /// Opens `page` (switching files if needed), moves the cursor to
    /// `line`, and selects it, scrolling it into view. The spot being
    /// left goes onto the back stack.
    pub fn open(&mut self, page: Page, line: usize) {
        let from = (self.page, self.content.cursor().position.line);
        self.history.push(from);
        if self.history.len() > HISTORY_DEPTH {
            self.history.remove(0);
        }
        // A fresh jump forks the timeline, browser-style.
        self.future.clear();
        self.show(page, line);
    }

    /// Whether [`State::back`] has anywhere to go.
    pub fn can_go_back(&self) -> bool {
        !self.history.is_empty()
    }

    /// Whether [`State::forward`] has anywhere to go.
    pub fn can_go_forward(&self) -> bool {
        !self.future.is_empty()
    }

    /// Returns to where the last jump came from.
    pub fn back(&mut self) {
        if let Some((page, line)) = self.history.pop() {
            self.future
                .push((self.page, self.content.cursor().position.line));
            self.show(page, line);
        }
    }

    /// Un-does a [`State::back`].
    pub fn forward(&mut self) {
        if let Some((page, line)) = self.future.pop() {
            self.history
                .push((self.page, self.content.cursor().position.line));
            self.show(page, line);
        }
    }

    fn show(&mut self, page: Page, line: usize) {
        use text_editor::{Action, Motion};
        if page != self.page {
            self.page = page;
            self.content = text_editor::Content::with_text(page.source());
        }
        self.content.perform(Action::Move(Motion::DocumentStart));
        for _ in 0..line {
            self.content.perform(Action::Move(Motion::Down));
        }
        self.content.perform(Action::Select(Motion::End));
    }

    /// The pane: the current file's name over its source, rendered
    /// under the same highlighter and style as the code editor.
    pub fn view(&self) -> Element<'_, Message> {
        let header = container(
            text(self.page.file_name())
                .size(11)
                .font(theme::MONO)
                .style(theme::text::secondary),
        )
        .padding([2, 4]);

        let source = text_editor(&self.content)
            .height(Fill)
            .size(13)
            .font(theme::MONO)
            .on_action(Message::Viewed)
            .highlight_with::<highlight::Highlighter>((), |kind, theme| kind.format(theme))
            .style(theme::text_editor::code);

        column![header, source].spacing(4).into()
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

    /// The API contract: every name a script can touch, and the page
    /// its documentation lives on. A rename or addition in `script/`
    /// that forgets the reference fails here.
    const CONTRACT: &[(&str, Page)] = &[
        ("new", Page::Lib),
        ("update", Page::Lib),
        ("message", Page::Lib),
        ("dt", Page::Lib),
        ("elevators", Page::Lib),
        ("floors", Page::Lib),
        ("Command", Page::Lib),
        ("go_to_floor", Page::Lib),
        ("stop", Page::Lib),
        ("check_destination_queue", Page::Lib),
        ("set_destination_queue", Page::Lib),
        ("set_going_up_indicator", Page::Lib),
        ("set_going_down_indicator", Page::Lib),
        ("Elevator", Page::Elevator),
        ("destination_queue", Page::Elevator),
        ("current_floor", Page::Elevator),
        ("max_passenger_count", Page::Elevator),
        ("load_factor", Page::Elevator),
        ("is_full", Page::Elevator),
        ("destination_direction", Page::Elevator),
        ("pressed_floors", Page::Elevator),
        ("move_count", Page::Elevator),
        ("is_busy", Page::Elevator),
        ("is_moving", Page::Elevator),
        ("is_on_a_floor", Page::Elevator),
        ("going_up_indicator", Page::Elevator),
        ("going_down_indicator", Page::Elevator),
        ("idle", Page::Elevator),
        ("floor_button_pressed", Page::Elevator),
        ("passing_floor", Page::Elevator),
        ("stopped_at_floor", Page::Elevator),
        ("Floor", Page::Floor),
        ("floor_num", Page::Floor),
        ("level", Page::Floor),
        ("up_pressed", Page::Floor),
        ("down_pressed", Page::Floor),
        ("up_button_pressed", Page::Floor),
        ("down_button_pressed", Page::Floor),
    ];

    #[test]
    fn every_name_in_the_api_contract_resolves_to_its_page() {
        let misplaced: Vec<_> = CONTRACT
            .iter()
            .filter(|&&(name, page)| resolve(name, None).map(|(p, _)| p) != Some(page))
            .collect();
        assert!(misplaced.is_empty(), "wrong or missing: {misplaced:?}");
    }

    #[test]
    fn ambiguous_names_prefer_the_page_they_were_clicked_on() {
        let (page, _) = resolve("Event", Some(Page::Floor)).unwrap();
        assert_eq!(page, Page::Floor);
        let (page, _) = resolve("Event", Some(Page::Elevator)).unwrap();
        assert_eq!(page, Page::Elevator);
        let (page, _) = resolve("Event", None).unwrap();
        assert_eq!(page, Page::Elevator, "default preference order");
    }

    #[test]
    fn camel_case_variants_register_their_snake_case_event_names() {
        let (page, camel) = resolve("StoppedAtFloor", None).unwrap();
        let (_, snake) = resolve("stopped_at_floor", None).unwrap();
        assert_eq!(page, Page::Elevator);
        assert_eq!(camel, snake, "both spellings share the variant line");
    }

    #[test]
    fn definitions_resolve_to_their_own_lines() {
        let (page, line) = resolve("load_factor", None).unwrap();
        let text = page.source().lines().nth(line).unwrap();
        assert!(text.contains("load_factor"));
        assert!(!text.trim_start().starts_with("//"));
    }

    #[test]
    fn identifiers_resolve_under_and_just_after_the_word() {
        let line = "    pub fn go_to_floor(&self, n: i64);";
        assert_eq!(identifier_in(line, 13).as_deref(), Some("go_to_floor"));
        assert_eq!(identifier_in(line, 22).as_deref(), Some("go_to_floor"));
        assert_eq!(identifier_in(line, 2), None);
    }

    #[test]
    fn forward_undoes_back_until_a_fresh_jump_forks_the_timeline() {
        let mut state = State::new();
        let (page, line) = resolve("go_to_floor", None).unwrap();
        state.open(page, line);
        let (page, line) = resolve("floor_num", None).unwrap();
        state.open(page, line);
        state.back();
        assert!(state.can_go_forward());
        state.forward();
        assert_eq!(state.page(), Page::Floor);
        state.back();
        let (page, line) = resolve("new", None).unwrap();
        state.open(page, line);
        assert!(!state.can_go_forward(), "a fresh jump clears forward");
    }

    #[test]
    fn back_returns_through_the_jump_history() {
        let mut state = State::new();
        assert!(!state.can_go_back());
        let (page, line) = resolve("load_factor", None).unwrap();
        state.open(page, line);
        let (page, line) = resolve("floor_num", None).unwrap();
        state.open(page, line);
        assert_eq!(state.page(), Page::Floor);
        state.back();
        assert_eq!(state.page(), Page::Elevator);
        state.back();
        assert_eq!(state.page(), Page::Lib);
        assert!(!state.can_go_back());
    }

    #[test]
    fn opening_a_page_lands_the_cursor_on_the_requested_line() {
        let mut state = State::new();
        let (page, line) = resolve("stopped_at_floor", None).unwrap();
        state.open(page, line);
        assert_eq!(state.page(), Page::Elevator);
        assert_eq!(state.content.cursor().position.line, line);
    }
}
