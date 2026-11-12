//! The library's pages - `lib.rs`, `building.rs`, `elevator.rs`,
//! `floor.rs`, Rust-voiced - rendered read-only through the same
//! highlighter as the code pane, with the history of cmd+click's jumps
//! between them. [`crate::resolution`] decides where a jump lands.

use iced::widget::text_editor;
use iced::{Element, Fill};

use crate::action::Action;
use crate::highlight;
use crate::resolution::{self, Scope};
use crate::theme;

/// One page of the reference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Page {
    /// Program shape: `run`, `new`, `update`, `subscription`, riders,
    /// determinism.
    Lib,
    /// `Building`: the floors and where the cars are.
    Building,
    /// `Elevator`, its commands, and its messages.
    Elevator,
    /// What riders on a floor do.
    Floor,
}

impl Page {
    /// Every page, in resolution-preference order for editor clicks.
    pub const ALL: &[Self] = &[Self::Elevator, Self::Floor, Self::Building, Self::Lib];

    /// The page's source text.
    pub fn source(self) -> &'static str {
        match self {
            Self::Lib => include_str!("../assets/api/lib.rs"),
            Self::Building => include_str!("../assets/api/building.rs"),
            Self::Elevator => include_str!("../assets/api/elevator.rs"),
            Self::Floor => include_str!("../assets/api/floor.rs"),
        }
    }

    /// The page's file name in the `api/` folder.
    pub fn file_name(self) -> &'static str {
        match self {
            Self::Lib => "lib.rs",
            Self::Building => "building.rs",
            Self::Elevator => "elevator.rs",
            Self::Floor => "floor.rs",
        }
    }
}

/// `text` when it is exactly one identifier - what a double click (or
/// double tap) leaves selected on a name.
pub fn identifier(text: &str) -> Option<String> {
    let word = text.trim();
    (!word.is_empty()
        && !word.starts_with(|c: char| c.is_ascii_digit())
        && word.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
    .then(|| word.to_owned())
}

/// The docs pane's state.
pub struct State {
    page: Page,
    content: text_editor::Content,
    /// Where cmd+clicks came from, newest last - the back stack.
    history: Vec<(Page, usize)>,
    /// Where back() came from, newest last - the forward stack.
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

    /// Applies a viewer message. Edits are silently dropped - the
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

    /// The definition the name under the cursor (set by the click just
    /// performed) names.
    pub fn reference_at_cursor(&self) -> Option<(Page, usize)> {
        let position = self.content.cursor().position;
        resolution::resolve(Scope::Page(self.page), position.line, position.column)
    }

    /// The selection, when it is exactly one identifier, and the
    /// definition it names - the double-tap jump affordance's input.
    pub fn selected_reference(&self) -> Option<(String, Page, usize)> {
        let name = identifier(&self.content.selection()?)?;
        let (page, line) = self.reference_at_cursor()?;
        Some((name, page, line))
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

    /// Where `path`, written in a program that imports the starter's
    /// names, resolves.
    fn site(path: &str) -> (Page, usize) {
        let source =
            format!("use elevato::{{Building, Elevator, Message, elevator, floor}};\n{path}");
        resolution::resolve(Scope::Program(&source), 1, path.len())
            .unwrap_or_else(|| panic!("{path} resolves"))
    }

    #[test]
    fn forward_undoes_back_until_a_fresh_jump_forks_the_timeline() {
        let mut state = State::new();
        let (page, line) = site("Elevator::go_to_floor");
        state.open(page, line);
        let (page, line) = site("floor::Message::UpButtonPressed");
        state.open(page, line);
        state.back();
        assert!(state.can_go_forward());
        state.forward();
        assert_eq!(state.page(), Page::Floor);
        state.back();
        let (page, line) = site("Elevator::new");
        state.open(page, line);
        assert!(!state.can_go_forward(), "a fresh jump clears forward");
    }

    #[test]
    fn back_returns_through_the_jump_history() {
        let mut state = State::new();
        assert!(!state.can_go_back());
        let (page, line) = site("Elevator::stop");
        state.open(page, line);
        let (page, line) = site("floor::Message::UpButtonPressed");
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
        let (page, line) = site("elevator::Message::StoppedAtFloor");
        state.open(page, line);
        assert_eq!(state.page(), Page::Elevator);
        assert_eq!(state.content.cursor().position.line, line);
    }
}
