//! Power on elevators in `new`, handle their messages and the riders'
//! in `update`, and return `Task`s to move them. `update` and `view`
//! read the building as it stands.
//!
//!     use elevato::{Building, Elevator, elevator, floor};
//!     use iced::{Element, Function, Subscription, Task};
//!
//! A car makes one trip at a time. It says `Idle` when it has nothing
//! to do; a trip is a task that streams the floors the car passes and
//! ends when it stops.

/// Opens the game with your program running the elevators. `main` calls
/// it once, with the program's four functions:
///
///     fn main() -> iced::Result {
///         elevato::run(Program::new, Program::update, Program::view, Program::subscription)
///     }
pub fn run(new, update, view, subscription) -> iced::Result;

/// Boots your program on a challenge. It learns how many elevators the
/// challenge's bank holds, powers on the ones it uses, and returns the
/// first tasks.
///
///     let (elevator, start) = Elevator::new();
///     (Self, start.map(Message::Elevator.with(elevator)))
pub fn new(elevators: usize) -> (Program, Task<Message>);

/// Handles one message, reading the building as it stands, and answers
/// with tasks. The elevators move when the tasks you return run, in the
/// order you return them.
pub fn update(&mut self, message: Message, building: &Building) -> Task<Message>;

/// Draws the building:
///
///     elevato::view(building)
pub fn view(&self, building: &Building) -> Element<'_, Message>;

/// Listens to the riders:
///
///     elevato::riders(Message::Elevator, Message::Floor)
pub fn subscription(&self) -> Subscription<Message>;

/// The building as it stands: its floors, the elevators you powered on,
/// and its riders.
pub struct Building;

impl Building {
    /// How many floors the building has; the lobby is floor 0.
    pub fn floors(&self) -> usize;

    /// The floor the car is at, or nearest to while it travels.
    pub fn floor_of(&self, elevator: Elevator) -> usize;
}

/// The building drawn: its floors and their call lamps, the elevators
/// you powered on, and the riders.
pub fn view<Message>(building: &Building) -> Element<'_, Message>;

/// Every button a rider presses that was not lit, made into your
/// messages: `elevator` for a destination pressed inside a car
/// (`FloorButtonPressed`), `floor` for a call on a floor
/// (`UpButtonPressed` or `DownButtonPressed`). A lit button stays
/// silent.
pub fn riders<M>(
    elevator: impl Fn(Elevator, elevator::Message) -> M,
    floor: impl Fn(usize, floor::Message) -> M,
) -> Subscription<M>;

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
