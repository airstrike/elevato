//! Power on an elevator with `Elevator::new`. Return a command's `Task`
//! to run it. Its news reaches your `update` as an
//! `elevato::Message::Elevator`, with the car and one of this module's
//! messages.

/// What a car tells your program.
pub enum Message {
    /// The car has nothing to do: on power-on, after its doors close with
    /// no trip waiting, or when `stop` halts it. Arrives through the task
    /// from `Elevator::new`.
    Idle,

    /// A rider presses an unlit destination button for this floor.
    /// Arrives through `riders()`, about a second after they board.
    FloorButtonPressed(usize),

    /// The car is about to pass this floor in the given direction.
    /// Streamed by the trip's task, in time to command a stop here.
    PassingFloor(usize, Direction),

    /// The trip's last message, sent at its destination after riders get
    /// off and on.
    StoppedAtFloor {
        /// Where the car stopped.
        floor: usize,
        /// How many riders got off.
        exited: usize,
        /// How many riders got on. Only riders whose direction has a lit
        /// indicator can board.
        boarded: usize,
        /// Total rider weight over capacity × 100: 0 empty, about 1 full.
        /// Riders weigh 55 to 100, so `full` is the exact test.
        load_factor: f64,
        /// Whether every place is taken.
        full: bool,
    },
}

/// A car of the challenge's bank. `Copy`, comparable, and hashable, so
/// you can use it as a key in your own maps.
pub struct Elevator;

impl Elevator {
    /// Powers on the bank's next car at the lobby. Its task sends `Idle`
    /// at once and whenever it has nothing to do. Cars beyond the bank's
    /// limit never enter the building or send messages.
    ///
    ///     let (elevator, start) = Elevator::new();
    pub fn new() -> (Elevator, Task<elevato::Message>);

    /// Powers on the bank's next car without its `Idle` messages.
    pub fn default() -> Elevator;

    /// Starts a trip to `floor`, replacing the current trip and ending
    /// its task without further messages. The new task streams
    /// `PassingFloor` and ends with `StoppedAtFloor`. Floors above the top
    /// are clamped to the top floor.
    ///
    ///     elevator.go_to_floor(3)
    pub fn go_to_floor(self, floor: usize) -> Task<elevato::Message>;

    /// Halts the car where it can - usually between floors, doors shut -
    /// and aborts its trip, whose task ends with no more messages. The
    /// car says `Idle`.
    pub fn stop<T>(self) -> Task<T>;

    /// Sets the up lamp. Both lamps start on. A car stopping with a lamp
    /// lit takes on riders going that way and clears that call button.
    pub fn set_going_up_indicator<T>(self, on: bool) -> Task<T>;

    /// Sets the down lamp. See `set_going_up_indicator`.
    pub fn set_going_down_indicator<T>(self, on: bool) -> Task<T>;
}
