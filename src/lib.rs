//! elevato - Elevator Saga in Rust: program a bank of elevators with an
//! iced program of your own, and watch the simulation clear (or fail)
//! the challenges.
//!
//! A program uses the API re-exported here - [`Message`], [`Elevator`],
//! [`Building`], the [`elevator`] and [`floor`] messages, [`riders()`]
//! and [`view()`] -
//! and hands its functions to [`run`], which opens the game with the
//! program running the elevators.

pub mod action;
pub mod app;
pub mod docs;
pub mod editor;
pub mod grade;
pub mod highlight;
pub mod icon;
pub mod playback;
pub mod reload;
pub mod resolution;
pub mod sim;
pub mod storage;
pub mod theme;
pub mod widget;

pub use app::run;
pub use elevato_api::{Building, Direction, Elevator, Message, elevator, floor, riders};
pub use elevato_core as core;

use iced::widget::canvas;
use iced::{Fill, Widget};

/// The building drawn: its floors and their call lamps, the cars the
/// program powered on, and the riders.
pub fn view<'a, Message: 'a>(building: &Building) -> impl Widget<Message> + 'a {
    canvas(sim::View::new(building.world().clone()))
        .width(Fill)
        .height(Fill)
}
