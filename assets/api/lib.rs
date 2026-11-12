//! Power on elevators in `new`, handle the building's messages in
//! `update`, and return `Task`s to move the cars. `update` and `view`
//! read the building as it stands.
//!
//!     use elevato::{Building, Elevator, Message, elevator, floor};
//!     use iced::{Subscription, Task, Widget};
//!
//! A car makes one trip at a time. It says `Idle` when it has nothing
//! to do; a trip is a task that streams the floors the car passes and
//! ends when it stops.

/// What happens in the building: news from a car, or a rider calling
/// one from a floor. Every command's task and `riders()` produce it, and
/// your `update` handles it.
pub enum Message {
    /// Something about a car: it is idle, a trip is under way, or a rider
    /// inside pressed a button.
    Elevator(Elevator, elevator::Message),

    /// A rider on this floor called a car.
    Floor(usize, floor::Message),
}

/// Opens the game with your program running the elevators. `main` calls
/// it once, with the program's four functions:
///
///     fn main() -> iced::Result {
///         elevato::run(Program::new, Program::update, Program::view, Program::subscription)
///     }
pub fn run(new, update, view, subscription) -> iced::Result;

/// Your program: `main.rs` writes these four, and `main` hands them to
/// `run`.
impl Program {
    /// Boots your program on a challenge. It learns how many elevators
    /// the challenge's bank holds, powers on the ones it uses, and returns
    /// the first tasks.
    ///
    ///     let (elevator, start) = Elevator::new();
    ///     (Self, start)
    pub fn new(elevators: usize) -> (Program, Task<Message>);

    /// Handles one message, reading the building as it stands, and
    /// answers with tasks. The elevators move when the tasks you return
    /// run, in the order you return them.
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

/// The building drawn: its floors and their call lamps, the elevators
/// you powered on, and the riders.
pub fn view<Message>(building: &Building) -> impl Widget<Message>;

/// Every button a rider presses that was not lit: a `Message::Floor`
/// for a call on a floor (`UpButtonPressed` or `DownButtonPressed`), a
/// `Message::Elevator` with `FloorButtonPressed` for a destination
/// pressed inside a car. A lit button stays silent.
pub fn riders() -> Subscription<Message>;

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
