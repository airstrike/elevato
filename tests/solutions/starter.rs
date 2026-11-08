// rawr
//! Your program. It is an iced program: `new` powers on the elevators it
//! uses, `update` handles each message and answers with tasks, `view`
//! draws the building, and `subscription` listens to the riders. `main`
//! hands the four to elevato, which runs the game around them.

use elevato::{Building, Elevator, elevator, floor};
use iced::{Element, Function, Subscription, Task};

pub fn main() -> iced::Result {
    elevato::run(
        Program::new,
        Program::update,
        Program::view,
        Program::subscription,
    )
}

/// Our state. We keep nothing so far.
pub struct Program;

/// Everything that can happen to us.
#[derive(Debug, Clone)]
pub enum Message {
    /// Something about an elevator: it is idle, a trip is under way, or
    /// a rider inside pressed a button.
    Elevator(Elevator, elevator::Message),
    /// A rider on a floor calling a car.
    Floor(usize, floor::Message),
}

impl Program {
    pub fn new(_elevators: usize) -> (Self, Task<Message>) {
        let (elevator, start) = Elevator::new();

        (Self, start.map(Message::Elevator.with(elevator)))
    }

    pub fn update(&mut self, message: Message, building: &Building) -> Task<Message> {
        match message {
            // Whenever the elevator is idle ...
            Message::Elevator(elevator, elevator::Message::Idle) => {
                // let's go to all the floors (or did we forget one?)
                let next = if building.floor_of(elevator) == 0 {
                    1
                } else {
                    0
                };

                elevator
                    .go_to_floor(next)
                    .map(Message::Elevator.with(elevator))
            }
            // We normally don't need to do anything here
            Message::Elevator(..) | Message::Floor(..) => Task::none(),
        }
    }

    pub fn view(&self, building: &Building) -> Element<'_, Message> {
        elevato::view(building)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        elevato::riders(Message::Elevator, Message::Floor)
    }
}
