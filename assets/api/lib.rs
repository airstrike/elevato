//! Power on elevators in `new`, handle the building's messages in `update`, and
//! return `Task`s to move the cars. `update` and `view` read the building as it
//! stands.
//!
//!     use elevato::{Building, Elevator, Message, elevator, floor};
//!     use iced::{Subscription, Task, Widget};
//!
//! A car makes one trip at a time. It says `Idle` when it has nothing to do; a
//! trip is a task that streams the floors the car passes and ends when it
//! stops.

/// Messages from car and trip tasks and `riders()`, handled by your `update`.
/// Stop and indicator tasks produce no messages themselves.
pub enum Message {
    /// Something about a car: it is idle, a trip is under way, or a rider
    /// inside pressed a button.
    Elevator(Elevator, elevator::Message),

    /// A rider on this floor called a car.
    Floor(usize, floor::Message),
}

/// Opens the game with your program running the elevators. `main` calls it
/// once, with the program's four functions:
///
///     fn main() -> iced::Result {
///         elevato::run(Program::new, Program::update, Program::view, Program::subscription)
///     }
pub fn run(new, update, view, subscription) -> iced::Result;

/// Define these four functions in `main.rs` and pass them to `run`.
impl Program {
    /// Receives the challenge's bank size. Power on the cars you need and
    /// return their tasks to receive messages.
    ///
    ///     let (elevator, start) = Elevator::new();
    ///     (Self, start)
    pub fn new(elevators: usize) -> (Program, Task<Message>);

    /// Handles one message and reads the building's current state. Movement and
    /// indicator commands take effect when their tasks run, in the order you
    /// return them.
    pub fn update(&mut self, message: Message, building: &Building) -> Task<Message>;

    /// Draws the building:
    ///
    ///     elevato::view(building)
    pub fn view(&self, building: &Building) -> impl Widget<Message>;

    /// Listens to the riders:
    ///
    ///     elevato::riders()
    pub fn subscription(&self) -> Subscription<Message>;
}

/// The building drawn: its floors and their call lamps, the elevators you
/// powered on, and the riders.
pub fn view<Message>(building: &Building) -> impl Widget<Message>;

/// Reports presses of unlit buttons: `Message::Floor` for a call on a floor
/// (`UpButtonPressed` or `DownButtonPressed`), `Message::Elevator` with
/// `FloorButtonPressed` for a destination inside a car. A lit button stays
/// silent.
pub fn riders() -> Subscription<Message>;

/// A direction of travel.
pub enum Direction {
    Up,
    Down,
}

// A panic anywhere in your code - `panic!`, a failed `assert!`, an index out of
// bounds - ends the attempt; its message shows under the editor.
//
// There is no clock: a task that waits on time or I/O never finishes. Runs are
// deterministic - same seed, same program, same result. The stats bar's Seed
// names the attempt.
