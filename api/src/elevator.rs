//! A car of the bank: the commands a program gives it, and the messages
//! it answers with.

use iced_runtime::Task;
use iced_runtime::futures::futures::channel::mpsc;

use crate::Direction;
use crate::hub::{self, Command};

/// A car of the challenge's bank.
///
/// Only a challenge makes elevators: a program gets them from its `new`
/// and from messages, so it can never name a car the building lacks.
/// Each command returns a [`Task`], and nothing happens until the
/// program hands that task back from `new` or `update`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Elevator(usize);

/// What a car tells the program.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Message {
    /// The car has no trip left to make: at the start, and after it stops
    /// at the last floor it was sent to. It arrives through
    /// [`riders()`](crate::riders).
    Idle,
    /// A rider inside pressed the button for this floor, and it was not
    /// lit. It arrives through [`riders()`](crate::riders).
    FloorButtonPressed(usize),
    /// The car is about to pass this floor without stopping, heading in
    /// this direction. A trip's task streams one per floor it passes,
    /// still in time for [`Elevator::go_to_floor_first`] to stop there.
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

impl Elevator {
    /// The car at this index of the challenge's bank.
    pub(crate) fn new(index: usize) -> Self {
        Self(index)
    }

    /// Its index in the challenge's bank.
    pub(crate) fn index(self) -> usize {
        self.0
    }

    /// A trip to `floor`, queued after the car's other trips. A floor
    /// past the top is the top floor, as in the original.
    ///
    /// The task streams [`Message::PassingFloor`] while the car travels
    /// toward `floor` and ends with [`Message::StoppedAtFloor`] when it
    /// gets there. Asking for the floor the car is already queued to
    /// reach last adds no stop: both trips end at that one arrival.
    pub fn go_to_floor(self, floor: usize) -> Task<Message> {
        self.trip(floor, false)
    }

    /// A trip to `floor`, ahead of the car's other trips: go there first.
    /// The trips it overtakes resume after it, each still running.
    pub fn go_to_floor_first(self, floor: usize) -> Task<Message> {
        self.trip(floor, true)
    }

    /// Stops the car where it can, usually between floors, and aborts
    /// every trip it was on: their tasks end with no further message.
    pub fn stop(self) -> Task<Message> {
        hub::command(Command::Stop { elevator: self })
    }

    /// Lights or darkens the up indicator. Both start lit. A car stopping
    /// with its up indicator lit takes on riders going up and clears the
    /// floor's up call.
    pub fn set_going_up_indicator(self, on: bool) -> Task<Message> {
        hub::command(Command::GoingUpIndicator { elevator: self, on })
    }

    /// Lights or darkens the down indicator; the twin of
    /// [`Elevator::set_going_up_indicator`].
    pub fn set_going_down_indicator(self, on: bool) -> Task<Message> {
        hub::command(Command::GoingDownIndicator { elevator: self, on })
    }

    fn trip(self, floor: usize, first: bool) -> Task<Message> {
        let (sender, receiver) = mpsc::unbounded();
        let (trip, handle) = Task::stream(receiver).abortable();
        hub::command(Command::GoToFloor {
            elevator: self,
            floor,
            first,
            sender,
            handle,
        })
        .chain(trip)
    }
}
