//! Power on an elevator with `Elevator::new`. Return a command's `Task`
//! to run it.

/// A car of the challenge's bank. `Copy`, comparable, hashable: a
/// fine key for your own maps.
pub struct Elevator;

impl Elevator {
    /// Powers on the bank's next car, standing at the lobby, with the
    /// task of its news: `Idle` at once, and whenever it has nothing left
    /// to do. A car past the bank never enters the building.
    ///
    ///     let (elevator, start) = Elevator::new();
    pub fn new() -> (Elevator, Task<Message>);

    /// Powers on the bank's next car without its news.
    pub fn default() -> Elevator;

    /// A trip to `floor`, replacing the car's current trip. The task
    /// streams `PassingFloor` while the car travels and ends with
    /// `StoppedAtFloor` when it gets there; a replaced trip's task ends
    /// with no more messages. A floor past the top is the top floor.
    ///
    ///     elevator.go_to_floor(3).map(Message::Elevator.with(elevator))
    pub fn go_to_floor(self, floor: usize) -> Task<Message>;

    /// Halts the car where it can - usually between floors, doors shut -
    /// and aborts its trip, whose task ends with no more messages. The
    /// car says `Idle`.
    pub fn stop(self) -> Task<Message>;

    /// Sets the up lamp. Both lamps start on. A car stopping with a lamp
    /// lit takes on riders going that way and clears that call button.
    pub fn set_going_up_indicator(self, on: bool) -> Task<Message>;

    /// Sets the down lamp. See `set_going_up_indicator`.
    pub fn set_going_down_indicator(self, on: bool) -> Task<Message>;
}

/// What a car tells your program.
pub enum Message {
    /// The car has nothing to do: when it is powered on, and whenever it
    /// finishes a trip or stops. Arrives through the task from
    /// `Elevator::new`.
    Idle,

    /// A rider inside pressed this floor's button, and it was not lit.
    /// Arrives through `riders()`, about a second after they board.
    FloorButtonPressed(usize),

    /// The car is about to pass this floor without stopping, going this
    /// way. Streamed by the trip's task, still in time to send the car
    /// there instead.
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
