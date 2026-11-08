//! Get elevators from `new` or messages. Return a command's `Task` to
//! run it.

/// A car of the challenge's bank. `Copy`, comparable, hashable: a
/// fine key for your own maps.
pub struct Elevator;

impl Elevator {
    /// A trip to `floor`, after the car's other trips. The task streams
    /// `PassingFloor` while the car travels and ends with
    /// `StoppedAtFloor` when it gets there. A floor past the top is the
    /// top floor. Asking for the floor the car already goes to last
    /// adds no stop: both trips end at that one arrival.
    ///
    ///     elevator.go_to_floor(3).map(Message::Elevator.with(elevator))
    pub fn go_to_floor(self, floor: usize) -> Task<Message>;

    /// A trip to `floor` ahead of the car's other trips: go there
    /// first. The trips it overtakes resume after it. In time from
    /// `PassingFloor`, it stops the car at the floor it is passing.
    pub fn go_to_floor_first(self, floor: usize) -> Task<Message>;

    /// Halts the car where it can - usually between floors, doors shut -
    /// and aborts its trips: their tasks end with no more messages.
    pub fn stop(self) -> Task<Message>;

    /// Sets the up lamp. Both lamps start on. A car stopping with a lamp
    /// lit takes on riders going that way and clears that call button.
    pub fn set_going_up_indicator(self, on: bool) -> Task<Message>;

    /// Sets the down lamp. See `set_going_up_indicator`.
    pub fn set_going_down_indicator(self, on: bool) -> Task<Message>;
}

/// What a car tells your program.
pub enum Message {
    /// The car has no trip left: at the start, and when it stops at the
    /// last floor it was sent to. Arrives through `riders()`.
    Idle,

    /// A rider inside pressed this floor's button, and it was not lit.
    /// Arrives through `riders()`, about a second after they board.
    FloorButtonPressed(usize),

    /// The car is about to pass this floor without stopping, going this
    /// way. Streamed by the trip's task, still in time for
    /// `go_to_floor_first` to stop there.
    PassingFloor(usize, Direction),

    /// The car reached the trip's floor: the trip task's last message,
    /// sent once riders have got off and on.
    StoppedAtFloor {
        /// Where the car stopped.
        floor: usize,
        /// How many riders got off.
        exited: usize,
        /// How many riders got on. Only riders going the way a lit lamp
        /// points board.
        boarded: usize,
        /// Riders' weight over capacity × 100: 0 empty, about 1 full.
        /// Riders weigh 55 to 100, so `full` is the exact test.
        load_factor: f64,
        /// Whether every place is taken.
        full: bool,
    },
}
