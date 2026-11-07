//! elevato's API: what a player's program uses to steer a bank of
//! elevators. The game re-exports it at its root, so programs name it
//! `elevato`.
//!
//! A program is an iced program. Its `new` receives the challenge's
//! elevators and floors and keeps them; its `update` folds each
//! [`Message`](elevator::Message) into its own state and answers with
//! [`Task`]s; its `subscription` listens to [`riders()`]. Nothing a
//! program calls reads the simulation: everything it knows arrives as a
//! message.
//!
//! - A car moves when the program runs the [`Task`] that
//!   [`Elevator::go_to_floor`] returns. The task streams
//!   [`elevator::Message::PassingFloor`] while the car travels and ends
//!   with [`elevator::Message::StoppedAtFloor`], which says who got off
//!   and who got on.
//! - Riders are the only other source of messages: a rider calling a car
//!   from a floor arrives as a [`floor::Message`], a rider pressing a
//!   button inside a car as an [`elevator::Message`], both through
//!   [`riders()`].
//!
//! [`Simulation`] is the host's side: it runs a program against a
//! challenge's world, deterministically, one frame at a time. The
//! starter program in the game's `src/main.rs` shows a whole program.

pub mod elevator;
pub mod floor;
pub mod simulation;

mod hub;

pub use elevato_core::event::Direction;
pub use elevator::Elevator;
pub use floor::Floor;
pub use simulation::Simulation;

use iced_futures::Subscription;
#[cfg(doc)]
use iced_runtime::Task;

/// Something a rider did: called a car from a floor, or pressed a
/// destination button inside one.
#[derive(Debug, Clone, PartialEq)]
pub enum Rider {
    /// A rider at this floor pressed a call button that was not lit.
    Floor(Floor, floor::Message),
    /// A rider in this car pressed a destination button that was not lit.
    Elevator(Elevator, elevator::Message),
}

/// What the riders do, as they do it: every call button pressed on a
/// floor and every destination button pressed in a car. A button that is
/// already lit stays silent.
pub fn riders() -> Subscription<Rider> {
    Subscription::run(hub::riders)
}
