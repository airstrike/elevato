//! Handle elevator and rider messages in `update`. Return `Task`s to
//! control the elevators.
//!
//!     use elevato::{Elevator, Rider, elevator, floor};
//!     use iced::{Function, Subscription, Task};
//!
//! A car's trip is a task: it streams the floors the car passes and
//! ends when the car stops. Riders are the other source of messages,
//! through `riders()`.

/// Opens the game with your program steering the elevators. `main`
/// calls it once, with the program's three functions:
///
///     fn main() -> iced::Result {
///         elevato::run(Program::new, Program::update, Program::subscription)
///     }
pub fn run(new, update, subscription) -> iced::Result;

/// Boots your program on a challenge. It gets the bank of elevators
/// and how many floors the building has, keeps what it needs, and
/// returns the first tasks - the moves the cars make before anything
/// happens.
///
///     let start = Task::batch(elevators.iter().map(|&elevator| {
///         elevator.go_to_floor(0).map(Message::Elevator.with(elevator))
///     }));
///     (Self { elevators, floors }, start)
pub fn new(elevators: Vec<Elevator>, floors: usize) -> (Program, Task<Message>);

/// Handles one message and answers with tasks. The elevators move
/// when the tasks you return run, in the order you return them.
pub fn update(&mut self, message: Message) -> Task<Message>;

/// Listens to the riders:
///
///     elevato::riders().map(|rider| match rider {
///         Rider::Elevator(elevator, message) => Message::Elevator(elevator, message),
///         Rider::Floor(floor, message) => Message::Floor(floor, message),
///     })
pub fn subscription(&self) -> Subscription<Message>;

/// Every button a rider presses that was not lit: a call on a floor,
/// or a destination inside a car. A lit button stays silent.
pub fn riders() -> Subscription<Rider>;

/// Something a rider did.
pub enum Rider {
    /// A rider on this floor called a car: `UpButtonPressed` or
    /// `DownButtonPressed`.
    Floor(usize, floor::Message),

    /// A rider in this car pressed a destination button:
    /// `FloorButtonPressed`.
    Elevator(Elevator, elevator::Message),
}

/// A direction of travel.
pub enum Direction {
    Up,
    Down,
}

// A panic anywhere in your code - `panic!`, a failed `assert!`, an
// index out of bounds - ends the attempt; its message shows under the
// editor.
//
// There is no clock: a task that waits on time or I/O never finishes.
// Runs are deterministic - same seed, same program, same result. The
// stats bar's Seed names the attempt.
