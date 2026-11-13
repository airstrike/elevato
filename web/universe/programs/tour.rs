// rawr
//! Exercises stop commands, indicators, arrival counts, and a custom view
//! so the browser runner supports those calls. Cars sweep the building
//! and serve destination buttons; one spare car has no idle listener.

use elevato::{Building, Direction, Elevator, Message, elevator, floor};
use iced::widget::{column, text};
use iced::{Fill, Subscription, Task, Widget};

pub fn main() -> iced::Result {
    elevato::run(
        Program::new,
        Program::update,
        Program::view,
        Program::subscription,
    )
}

pub struct Program {
    cars: Vec<Elevator>,
    spare: Option<Elevator>,
    moved: usize,
}

impl Program {
    pub fn new(elevators: usize) -> (Self, Task<Message>) {
        let mut cars = Vec::new();
        let mut starts = Vec::new();
        for _ in 1..elevators {
            let (elevator, start) = Elevator::new();
            cars.push(elevator);
            starts.push(start);
        }
        let spare = if elevators > 0 {
            Some(Elevator::default())
        } else {
            None
        };
        let program = Self {
            cars,
            spare,
            moved: 0,
        };
        (program, Task::batch(starts))
    }

    pub fn update(&mut self, message: Message, building: &Building) -> Task<Message> {
        match message {
            Message::Elevator(elevator, elevator::Message::Idle) => {
                let top = building.floors() - 1;
                let next = if building.floor_of(elevator) < top {
                    top
                } else {
                    0
                };
                let going_up = next > building.floor_of(elevator);
                Task::batch([
                    elevator.set_going_up_indicator(going_up),
                    elevator.set_going_down_indicator(!going_up),
                    elevator.go_to_floor(next),
                ])
            }
            Message::Elevator(
                elevator,
                elevator::Message::StoppedAtFloor {
                    floor,
                    exited,
                    boarded,
                    load_factor,
                    full,
                },
            ) => {
                self.moved += exited;
                if full || load_factor > 0.9 || (boarded == 0 && floor == 0) {
                    elevator.stop()
                } else {
                    Task::none()
                }
            }
            Message::Elevator(elevator, elevator::Message::PassingFloor(floor, direction)) => {
                match direction {
                    Direction::Up if floor + 1 == building.floors() => elevator.stop(),
                    Direction::Up | Direction::Down => Task::none(),
                }
            }
            Message::Elevator(elevator, elevator::Message::FloorButtonPressed(floor)) => {
                if self.cars.contains(&elevator) && Some(elevator) != self.spare {
                    elevator.go_to_floor(floor)
                } else {
                    Task::none()
                }
            }
            Message::Floor(_, floor::Message::UpButtonPressed)
            | Message::Floor(_, floor::Message::DownButtonPressed) => Task::none(),
        }
    }

    pub fn view(&self, building: &Building) -> impl Widget<Message> {
        column![
            text(format!("{} moved", self.moved)).size(12),
            elevato::view(building),
        ]
        .spacing(4)
        .width(Fill)
        .height(Fill)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        elevato::riders()
    }
}
