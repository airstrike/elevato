//! A car of the bank: the commands a program gives it, and the messages
//! it answers with.

use iced_runtime::Task;
use iced_runtime::futures::futures::channel::mpsc;

use crate::hub::{self, Command};
use crate::{Direction, message};

/// What a car tells the program.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Message {
    /// The car has nothing to do: when it is powered on, and whenever it
    /// finishes a trip or a [`Elevator::stop`] halts it. It arrives through
    /// the task [`Elevator::new`] returns.
    Idle,
    /// A rider inside pressed the button for this floor, and it was not
    /// lit. It arrives through [`riders()`](crate::riders).
    FloorButtonPressed(usize),
    /// The car is about to pass this floor without stopping, heading in
    /// this direction. A trip's task streams one per floor it passes.
    PassingFloor(usize, Direction),
    /// The car stopped at the floor its trip was going to: the last
    /// message of that trip's task, sent once riders have got off and on.
    StoppedAtFloor {
        /// Where the car stopped.
        floor: usize,
        /// How many riders got off here.
        exited: usize,
        /// How many riders got on here.
        boarded: usize,
        /// The riders' weight over the car's capacity: 0 when empty,
        /// about 1 when full.
        load_factor: f64,
        /// Whether every place in the car is taken.
        full: bool,
    },
}

/// A car of the challenge's bank.
///
/// [`Elevator::new`] powers on the bank's next car. Each command returns
/// a [`Task`] of the program's [`Message`](message::Message), and nothing
/// happens before the program hands that task back from `new` or
/// `update`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Elevator(usize);

impl Elevator {
    /// Powers on the bank's next car, standing at the lobby, and returns
    /// it with the task that streams its news: [`Message::Idle`] at once,
    /// and again whenever it has nothing left to do.
    ///
    /// The challenge's bank holds as many cars as `new` was told. A car
    /// asked for past that never enters the building: it says nothing,
    /// and its commands do nothing.
    pub fn new() -> (Self, Task<message::Message>) {
        let (sender, receiver) = mpsc::unbounded();
        let elevator = hub::power_on(Some(sender));
        (elevator, elevator.news(Task::stream(receiver)))
    }

    /// The car at this index of the bank.
    pub(crate) fn at(index: usize) -> Self {
        Self(index)
    }

    /// Its index in the bank.
    pub(crate) fn index(self) -> usize {
        self.0
    }

    /// The car's own news, as the program's messages about this car.
    fn news(self, task: Task<Message>) -> Task<message::Message> {
        task.map(move |message| message::Message::Elevator(self, message))
    }

    /// A trip to `floor`, replacing the car's current trip, whose task
    /// then ends with no further message. A floor past the top is the top
    /// floor, as in the original.
    ///
    /// The task streams [`Message::PassingFloor`] while the car travels
    /// and ends with [`Message::StoppedAtFloor`] when it gets there.
    pub fn go_to_floor(self, floor: usize) -> Task<message::Message> {
        let (sender, receiver) = mpsc::unbounded();
        let (trip, handle) = Task::stream(receiver).abortable();
        hub::command(Command::GoToFloor {
            elevator: self,
            floor,
            sender,
            handle,
        })
        .chain(self.news(trip))
    }

    /// Stops the car where it can, usually between floors, and aborts its
    /// trip: that task ends with no further message, and the car reports
    /// [`Message::Idle`].
    pub fn stop(self) -> Task<message::Message> {
        hub::command(Command::Stop { elevator: self })
    }

    /// Lights or darkens the up indicator. Both start lit. A car stopping
    /// with its up indicator lit takes on riders going up and clears the
    /// floor's up call.
    pub fn set_going_up_indicator(self, on: bool) -> Task<message::Message> {
        hub::command(Command::GoingUpIndicator { elevator: self, on })
    }

    /// Lights or darkens the down indicator; the twin of
    /// [`Elevator::set_going_up_indicator`].
    pub fn set_going_down_indicator(self, on: bool) -> Task<message::Message> {
        hub::command(Command::GoingDownIndicator { elevator: self, on })
    }
}

/// Powers on the bank's next car without listening to its news.
impl Default for Elevator {
    fn default() -> Self {
        hub::power_on(None)
    }
}
