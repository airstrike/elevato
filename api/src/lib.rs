//! elevato's API: what a player's program uses to run a bank of
//! elevators. The game re-exports it at its root, so programs name it
//! `elevato`.
//!
//! A program is an iced program. Its `new` learns how many cars the
//! challenge's bank holds and powers on the ones it wants with
//! [`Elevator::new`]; its `update` folds each
//! [`Message`](elevator::Message) into its own state, reading the
//! [`Building`] as it stands, and answers with [`Task`]s; its `view`
//! draws the building; its `subscription` listens to [`riders()`].
//!
//! - A car's own task, from [`Elevator::new`], says
//!   [`elevator::Message::Idle`] whenever the car has nothing to do.
//! - A car moves when the program runs the [`Task`] that
//!   [`Elevator::go_to_floor`] returns: one trip at a time. The task
//!   streams [`elevator::Message::PassingFloor`] while the car travels and
//!   ends with [`elevator::Message::StoppedAtFloor`], which says who got
//!   off and who got on.
//! - Riders are the other source of messages: a rider calling a car from
//!   a floor arrives as a [`floor::Message`], a rider pressing a button
//!   inside a car as an [`elevator::Message`], both through [`riders()`].
//!
//! [`Simulation`] is the host's side: it runs a program against a
//! challenge's world, deterministically, one frame at a time. The
//! starter program in `program/src/main.rs` shows a whole program.

pub mod building;
pub mod elevator;
pub mod floor;
pub mod simulation;

mod hub;

pub use building::Building;
pub use elevato_core::event::Direction;
pub use elevator::Elevator;
pub use simulation::Simulation;

use std::any::TypeId;
use std::hash::{Hash, Hasher};

use iced_futures::futures::StreamExt;
use iced_futures::{MaybeSend, Subscription};
#[cfg(doc)]
use iced_runtime::Task;

/// What the riders do, as they do it, made into the program's messages
/// by its constructors: `elevator` for a destination button pressed in
/// a car, `floor` for a call button pressed on a floor. A button that is
/// already lit stays silent.
///
/// ```ignore
/// elevato::riders(Message::Elevator, Message::Floor)
/// ```
pub fn riders<M>(
    elevator: impl Fn(Elevator, elevator::Message) -> M + MaybeSend + Clone + 'static,
    floor: impl Fn(usize, floor::Message) -> M + MaybeSend + Clone + 'static,
) -> Subscription<M>
where
    M: MaybeSend + 'static,
{
    Subscription::run_with(Constructors { elevator, floor }, |constructors| {
        let Constructors { elevator, floor } = constructors.clone();
        hub::riders().map(move |rider| match rider {
            hub::Rider::Elevator(car, message) => elevator(car, message),
            hub::Rider::Floor(level, message) => floor(level, message),
        })
    })
}

/// The program's two constructors [`riders`] makes messages with.
#[derive(Clone)]
struct Constructors<E, F> {
    elevator: E,
    floor: F,
}

/// A program subscribes to the riders once, so its constructors' types
/// identify the subscription.
impl<E: 'static, F: 'static> Hash for Constructors<E, F> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        TypeId::of::<Self>().hash(state);
    }
}
