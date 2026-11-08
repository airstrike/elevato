// rawr
//! Your program. It is an iced program: `new` boots it with the
//! challenge's elevators and floors, `update` handles each message and
//! answers with tasks, and `subscription` listens to the riders. `main`
//! hands the three to elevato, which runs the game around them.
//!
//! Under `rawr run`, a save reaches the running game at its next
//! message, and the program keeps its state when the new code allows
//! it. Under `cargo run`, the next build compiles your edits.

use elevato::{Elevator, elevator, floor};
use iced::{Function, Subscription, Task};

pub fn main() -> iced::Result {
    elevato::run(Program::new, Program::update, Program::subscription)
}

/// Our state. All we keep so far is how many floors the building has.
pub struct Program {
    floors: usize,
}

/// Everything that can happen to us.
#[derive(Debug, Clone)]
pub enum Message {
    /// Something about an elevator: a trip under way, or a rider inside
    /// pressing a button.
    Elevator(Elevator, elevator::Message),
    /// A rider on a floor calling a car.
    Floor(usize, floor::Message),
}

impl Program {
    pub fn new(elevators: Vec<Elevator>, floors: usize) -> (Self, Task<Message>) {
        // Send every elevator to the lobby to begin with.
        let start = Task::batch(elevators.into_iter().map(|elevator| {
            elevator
                .go_to_floor(0)
                .map(Message::Elevator.with(elevator))
        }));
        (Self { floors }, start)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            // Whenever an elevator stops at a floor ...
            Message::Elevator(elevator, elevator::Message::StoppedAtFloor { floor, .. }) => {
                // ... let's go to all the floors (or did we forget one?)
                let next = (floor + 1) % self.floors.min(2);
                elevator
                    .go_to_floor(next)
                    .map(Message::Elevator.with(elevator))
            }
            // Everything else, we ignore for now.
            Message::Elevator(..) | Message::Floor(..) => Task::none(),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        elevato::riders(Message::Elevator, Message::Floor)
    }
}
