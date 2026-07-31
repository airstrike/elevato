//! The single application screen — an empty shell until the sim view lands.

use iced::widget::{center, text};
use iced::{Element, Task};

/// Top-level application state.
pub struct App;

/// Top-level application messages.
#[derive(Debug, Clone)]
pub enum Message {}

/// Builds the initial state and startup task.
pub fn boot() -> (App, Task<Message>) {
    (App, Task::none())
}

/// Applies a message to the state.
pub fn update(_app: &mut App, message: Message) -> Task<Message> {
    match message {}
}

/// Renders the screen.
pub fn view(_app: &App) -> Element<'_, Message> {
    center(text("elevato")).into()
}
