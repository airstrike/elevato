//! Where a program's tasks meet the simulation running them.
//!
//! [`Simulation`](crate::Simulation) makes its hub current on the thread while
//! running player code. Tasks submit commands on their first poll; car
//! allocation and [`riders`] subscriptions use the same hub.

use std::cell::RefCell;

use iced_runtime::futures::MaybeSend;
use iced_runtime::futures::futures::channel::mpsc;
use iced_runtime::{Task, task};

use crate::{Elevator, Message, elevator};

/// The commands submitted, the cars handed out, and the riders' subscribers.
#[derive(Debug, Default)]
pub(crate) struct Hub {
    commands: Vec<Command>,
    cars: usize,
    riders: Vec<mpsc::UnboundedSender<Message>>,
}

/// An instruction to one car, submitted by the task that carries it.
#[derive(Debug)]
pub(crate) enum Command {
    /// Put the bank's next car in the building. Its news (`Idle`) goes to
    /// `news`, when the program listens.
    PowerOn {
        elevator: Elevator,
        news: Option<mpsc::UnboundedSender<elevator::Message>>,
    },
    /// Start a trip, replacing the car's current one. The trip's messages go to
    /// `sender`; `handle` aborts its task.
    GoToFloor {
        elevator: Elevator,
        floor: usize,
        sender: mpsc::UnboundedSender<elevator::Message>,
        handle: task::Handle,
    },
    /// Halt and abort the trip.
    Stop { elevator: Elevator },
    /// Set the up indicator.
    GoingUpIndicator { elevator: Elevator, on: bool },
    /// Set the down indicator.
    GoingDownIndicator { elevator: Elevator, on: bool },
}

impl Command {
    pub(crate) fn elevator(&self) -> Elevator {
        match self {
            Self::PowerOn { elevator, .. }
            | Self::GoToFloor { elevator, .. }
            | Self::Stop { elevator }
            | Self::GoingUpIndicator { elevator, .. }
            | Self::GoingDownIndicator { elevator, .. } => *elevator,
        }
    }
}

thread_local! {
    static CURRENT: RefCell<Option<Hub>> = const { RefCell::new(None) };
}

/// Makes `hub` current for `f` and recovers it on return or panic.
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

/// A task that submits `command` when it first runs, and produces nothing.
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

/// Hands out the next car of the bank and submits its power-on, so the
/// simulation puts it in the building before the program runs again.
pub(crate) fn power_on(news: Option<mpsc::UnboundedSender<elevator::Message>>) -> Elevator {
    CURRENT.with_borrow_mut(|hub| {
        let hub = hub
            .as_mut()
            .expect("invariant: elevato's elevators are made inside a Simulation");
        let elevator = Elevator::at(hub.cars);
        hub.cars += 1;
        hub.commands.push(Command::PowerOn { elevator, news });
        elevator
    })
}

/// The commands submitted since the last call, oldest first.
pub(crate) fn commands() -> Vec<Command> {
    CURRENT.with_borrow_mut(|hub| {
        hub.as_mut()
            .map(|hub| std::mem::take(&mut hub.commands))
            .unwrap_or_default()
    })
}

/// Sends what a rider did to every live [`riders`] stream.
pub(crate) fn broadcast(rider: Message) {
    CURRENT.with_borrow_mut(|hub| {
        if let Some(hub) = hub {
            hub.riders
                .retain(|subscriber| subscriber.unbounded_send(rider).is_ok());
        }
    });
}

/// A new stream of riders, subscribed to the current hub. Outside one it ends
/// at once.
pub(crate) fn riders() -> mpsc::UnboundedReceiver<Message> {
    let (sender, receiver) = mpsc::unbounded();
    CURRENT.with_borrow_mut(|hub| {
        if let Some(hub) = hub {
            hub.riders.push(sender);
        }
    });
    receiver
}
