//! elevato - Elevator Saga in Rust: program a bank of elevators with an
//! iced program of your own, and watch the simulation clear (or fail)
//! the challenges.
//!
//! A program uses the API re-exported here - [`Elevator`], [`Floor`],
//! their [`elevator`] and [`floor`] messages and [`riders()`] - and
//! hands its functions to [`run`], which opens the game with the
//! program steering the elevators.

pub mod action;
pub mod app;
pub mod docs;
pub mod editor;
pub mod highlight;
pub mod icon;
pub mod playback;
pub mod reload;
pub mod sim;
pub mod storage;
pub mod theme;
pub mod widget;

pub use app::run;
pub use elevato_api::{Direction, Elevator, Floor, Rider, elevator, floor, riders};
pub use elevato_core as core;
