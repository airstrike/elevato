//! elevato's API: what a player's program uses to run a bank of
//! elevators. The game re-exports it at its root, so programs name it
//! `elevato`.
//!
//! An iced program controls the cars through [`Task`]s and [`Message`]s.
//! Its `new` receives the bank size and powers on cars with [`Elevator::new`].
//! Its `update` reads the [`Building`] and returns tasks; `view` draws the
//! building, and `subscription` listens to [`riders()`].
//!
//! - A car's own task, from [`Elevator::new`], says
//!   [`elevator::Message::Idle`] whenever the car has nothing to do.
//! - A car moves when the program runs the [`Task`] that
//!   [`Elevator::go_to_floor`] returns: one trip at a time. The task
//!   streams [`elevator::Message::PassingFloor`] while the car travels and
//!   ends with [`elevator::Message::StoppedAtFloor`], which reports how
//!   many riders get off and on.
//! - [`riders()`] reports call-button presses as [`Message::Floor`] and
//!   destination-button presses inside a car as [`Message::Elevator`].
//!
//! [`Simulation`] is the host's side: it runs a program against a
//! challenge's world, deterministically, one frame at a time. The
//! starter program in `program/src/main.rs` shows a whole program.

pub mod building;
pub mod elevator;
pub mod floor;
pub mod simulation;

mod hub;
mod message;

pub use building::Building;
pub use elevato_core::event::Direction;
pub use elevator::Elevator;
pub use message::Message;
pub use simulation::Simulation;

use iced_futures::Subscription;
#[cfg(doc)]
use iced_runtime::Task;

/// What the riders do, as they do it: a [`Message::Floor`] for a call
/// button pressed on a floor, a [`Message::Elevator`] with
/// [`elevator::Message::FloorButtonPressed`] for a destination pressed in
/// a car. A button that is already lit stays silent.
///
/// ```ignore
/// fn subscription(&self) -> Subscription<Message> {
///     elevato::riders()
/// }
/// ```
pub fn riders() -> Subscription<Message> {
    Subscription::run(hub::riders)
}
