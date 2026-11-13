//! A car of the bank: the commands a program gives it, and the messages
//! it answers with.

use iced_runtime::Task;
use iced_runtime::futures::MaybeSend;
use iced_runtime::futures::futures::channel::mpsc;

use crate::hub::{self, Command};
use crate::{Direction, message};

/// What a car tells the program.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Message {
    /// The car has nothing to do: on power-on, after its doors close with
    /// no trip waiting, or when [`Elevator::stop`] halts it. Arrives through
    /// the task [`Elevator::new`] returns.
    Idle,
    /// A rider presses an unlit destination button for this floor.
    /// Arrives through [`riders()`](crate::riders) after the boarding walk.
    FloorButtonPressed(usize),
    /// The car is about to pass this floor without stopping, heading in
    /// this direction. A trip's task streams one per floor it passes.
    PassingFloor(usize, Direction),
    /// The trip's last message, sent at its destination after riders get
    /// off and on.
    StoppedAtFloor {
        /// Where the car stopped.
        floor: usize,
        /// How many riders got off here.
        exited: usize,
        /// How many riders got on here.
        boarded: usize,
        /// Total rider weight over capacity × 100: 0 empty, about 1 full.
        /// Riders weigh 55 to 100, so `full` is the exact test.
        load_factor: f64,
        /// Whether every place in the car is taken.
        full: bool,
    },
}

/// A car of the challenge's bank.
///
/// [`Elevator::new`] powers on the bank's next car. Movement and indicator
/// commands run only when the program returns their [`Task`]s from `new`
/// or `update`. Car and trip tasks carry [`Message`](message::Message)s;
/// other command tasks carry none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Elevator(usize);

impl Elevator {
    /// Powers on the bank's next car, standing at the lobby, and returns
    /// it with the task that streams its news: [`Message::Idle`] at once,
    /// and again whenever it has nothing left to do.
    ///
    /// The bank size is the argument to the program's `new`. Cars beyond
    /// that limit never enter the building, send messages, or act on commands.
    pub fn new() -> (Self, Task<message::Message>) {
        let (sender, receiver) = mpsc::unbounded();
        let elevator = hub::power_on(Some(sender));
        (elevator, elevator.news(Task::stream(receiver)))
    }

    /// The car at this index of the bank.
    pub(crate) fn at(index: usize) -> Self {
        Self(index)
    }

    pub(crate) fn index(self) -> usize {
        self.0
    }

    /// The car's own news, as the program's messages about this car.
    fn news(self, task: Task<Message>) -> Task<message::Message> {
        task.map(move |message| message::Message::Elevator(self, message))
    }

    /// Starts a trip to `floor`, replacing the current trip and ending
    /// its task without further messages. Floors above the top are clamped
    /// to the top floor.
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
    pub fn stop<T: MaybeSend + 'static>(self) -> Task<T> {
        hub::command(Command::Stop { elevator: self })
    }

    /// Lights or darkens the up indicator. Both start lit. A car stopping
    /// with its up indicator lit takes on riders going up and clears the
    /// floor's up call.
    pub fn set_going_up_indicator<T: MaybeSend + 'static>(self, on: bool) -> Task<T> {
        hub::command(Command::GoingUpIndicator { elevator: self, on })
    }

    /// Lights or darkens the down indicator; the twin of
    /// [`Elevator::set_going_up_indicator`].
    pub fn set_going_down_indicator<T: MaybeSend + 'static>(self, on: bool) -> Task<T> {
        hub::command(Command::GoingDownIndicator { elevator: self, on })
    }
}

/// Powers on the bank's next car without listening to its news.
impl Default for Elevator {
    fn default() -> Self {
        hub::power_on(None)
    }
}
