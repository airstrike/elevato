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

/// Our state: the elevator we use, and how many trips it has queued.
pub struct Program {
    elevator: Elevator,
    trips: usize,
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
    pub fn new(elevators: Vec<Elevator>, _floors: usize) -> (Self, Task<Message>) {
        let elevator = elevators[0]; // Let's use the first elevator

        let start = Task::batch([elevator.go_to_floor(0), elevator.go_to_floor(1)])
            .map(Message::Elevator.with(elevator));
        (Self { elevator, trips: 2 }, start)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Elevator(elevator, elevator::Message::StoppedAtFloor { .. })
                if elevator == self.elevator =>
            {
                self.trips -= 1;

                // Whenever the elevator is idle (has no more queued destinations) ...
                if self.trips > 0 {
                    return Task::none();
                }

                // let's go to all the floors (or did we forget one?)
                self.trips = 2;
                Task::batch([elevator.go_to_floor(0), elevator.go_to_floor(1)])
                    .map(Message::Elevator.with(elevator))
            }
            // We normally don't need to do anything here
            Message::Elevator(..) | Message::Floor(..) => Task::none(),
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        elevato::riders(Message::Elevator, Message::Floor)
    }
}
