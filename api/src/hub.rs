//! Where a program's tasks meet the simulation running them.
//!
//! A [`Simulation`](crate::Simulation) enters its hub around everything
//! it runs of the program, on the thread that runs it. A command's task
//! submits the command here when the simulation first polls it, and
//! [`riders`] streams subscribe here when the simulation starts them.

use std::cell::RefCell;

use iced_runtime::futures::MaybeSend;
use iced_runtime::futures::futures::channel::mpsc;
use iced_runtime::{Task, task};

use crate::elevator::{self, Elevator};
use crate::floor;

/// The commands submitted and the riders' subscribers.
#[derive(Debug, Default)]
pub(crate) struct Hub {
    commands: Vec<Command>,
    riders: Vec<mpsc::UnboundedSender<Rider>>,
}

/// Something a rider did: pressed a call button on a floor, or a
/// destination button inside a car.
#[derive(Debug, Clone)]
pub(crate) enum Rider {
    /// A rider at this floor pressed a call button that was not lit.
    Floor(usize, floor::Message),
    /// A rider in this car pressed a destination button that was not lit.
    Elevator(Elevator, elevator::Message),
}

/// An instruction to one car, submitted by the task that carries it.
#[derive(Debug)]
pub(crate) enum Command {
    /// Queue a trip, at the front when `first`. The trip's messages go
    /// to `sender`; `handle` aborts its task.
    GoToFloor {
        elevator: Elevator,
        floor: usize,
        first: bool,
        sender: mpsc::UnboundedSender<elevator::Message>,
        handle: task::Handle,
    },
    /// Halt and abort every trip.
    Stop { elevator: Elevator },
    /// Set the up indicator.
    GoingUpIndicator { elevator: Elevator, on: bool },
    /// Set the down indicator.
    GoingDownIndicator { elevator: Elevator, on: bool },
}

thread_local! {
    static CURRENT: RefCell<Option<Hub>> = const { RefCell::new(None) };
}

/// Runs `f` with `hub` as the current one, and takes it back after,
/// whether `f` returns or panics.
pub(crate) fn enter<T>(hub: &mut Hub, f: impl FnOnce() -> T) -> T {
    struct Restore<'a>(&'a mut Hub);

    impl Drop for Restore<'_> {
        fn drop(&mut self) {
            if let Some(hub) = CURRENT.take() {
                *self.0 = hub;
            }
        }
    }

    CURRENT.set(Some(std::mem::take(hub)));
    let _restore = Restore(hub);
    f()
}

/// A task that submits `command` when it first runs, and produces
/// nothing.
pub(crate) fn command<T>(command: Command) -> Task<T>
where
    T: MaybeSend + 'static,
{
    Task::future(async move {
        CURRENT.with_borrow_mut(|hub| {
            hub.as_mut()
                .expect("invariant: elevato's tasks run inside a Simulation")
                .commands
                .push(command);
        });
    })
    .discard()
}

/// The commands submitted since the last call, oldest first.
pub(crate) fn commands() -> Vec<Command> {
    CURRENT.with_borrow_mut(|hub| {
        hub.as_mut()
            .map(|hub| std::mem::take(&mut hub.commands))
            .unwrap_or_default()
    })
}

/// Sends `rider` to every live [`riders`] stream.
pub(crate) fn broadcast(rider: Rider) {
    CURRENT.with_borrow_mut(|hub| {
        if let Some(hub) = hub {
            hub.riders
                .retain(|subscriber| subscriber.unbounded_send(rider.clone()).is_ok());
        }
    });
}

/// A new stream of riders, subscribed to the current hub. Outside one it
/// ends at once.
pub(crate) fn riders() -> mpsc::UnboundedReceiver<Rider> {
    let (sender, receiver) = mpsc::unbounded();
    CURRENT.with_borrow_mut(|hub| {
        if let Some(hub) = hub {
            hub.riders.push(sender);
        }
    });
    receiver
}
