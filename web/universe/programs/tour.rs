// rawr
//! The rest of the API, so the runner covers it: each car sweeps up and
//! down the whole building, stopping where it is when a car is full,
//! steering boarding with its indicators, and the view says how many
//! riders it moved above the building.

use elevato::{Building, Direction, Elevator, elevator, floor};
use iced::widget::{column, text};
use iced::{Element, Fill, Function, Subscription, Task};

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

#[derive(Debug, Clone)]
pub enum Message {
    Elevator(Elevator, elevator::Message),
    Floor(usize, floor::Message),
}

impl Program {
    pub fn new(elevators: usize) -> (Self, Task<Message>) {
        let mut cars = Vec::new();
        let mut starts = Vec::new();
        for _ in 1..elevators {
            let (elevator, start) = Elevator::new();
            cars.push(elevator);
            starts.push(start.map(Message::Elevator.with(elevator)));
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
                let next = if building.floor_of(elevator) < top { top } else { 0 };
                let going_up = next > building.floor_of(elevator);
                Task::batch([
                    elevator.set_going_up_indicator(going_up),
                    elevator.set_going_down_indicator(!going_up),
                    elevator.go_to_floor(next),
                ])
                .map(Message::Elevator.with(elevator))
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
                    elevator.stop().map(Message::Elevator.with(elevator))
                } else {
                    Task::none()
                }
            }
            Message::Elevator(elevator, elevator::Message::PassingFloor(floor, direction)) => {
                match direction {
                    Direction::Up if floor + 1 == building.floors() => {
                        elevator.stop().map(Message::Elevator.with(elevator))
                    }
                    Direction::Up | Direction::Down => Task::none(),
                }
            }
            Message::Elevator(elevator, elevator::Message::FloorButtonPressed(floor)) => {
                if self.cars.contains(&elevator) && Some(elevator) != self.spare {
                    elevator.go_to_floor(floor).map(Message::Elevator.with(elevator))
                } else {
                    Task::none()
                }
            }
            Message::Floor(_, floor::Message::UpButtonPressed)
            | Message::Floor(_, floor::Message::DownButtonPressed) => Task::none(),
        }
    }

    pub fn view(&self, building: &Building) -> Element<'_, Message> {
        column![
            text(format!("{} moved", self.moved)).size(12),
            elevato::view(building),
        ]
        .spacing(4)
        .width(Fill)
        .height(Fill)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        elevato::riders(Message::Elevator, Message::Floor)
    }
}
