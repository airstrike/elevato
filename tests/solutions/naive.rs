//! The wiki's "easy solution": every car serves its own riders' buttons and
//! returns to the lobby when idle. Every floor call goes to the first car. It
//! ignores direction and load, clearing only the first challenges.
//!
//! Each car is its own small iced component: [`Car`] owns the floors it has to
//! visit, its `update` takes an [`elevator::Message`], and the program routes
//! each car's messages to it. A car makes one trip at a time, the next one
//! whenever it is idle.

use std::collections::VecDeque;

use elevato::{Building, Elevator, Message, elevator};
use iced::{Subscription, Task, Widget};

pub struct Program {
    cars: Vec<Car>,
}

/// One car and the floors it still has to visit, in the order asked.
struct Car {
    elevator: Elevator,
    stops: VecDeque<usize>,
}

impl Program {
    pub fn new(elevators: usize) -> (Self, Task<Message>) {
        let (cars, starts): (Vec<Car>, Vec<Task<Message>>) = (0..elevators)
            .map(|_| {
                let (elevator, start) = Elevator::new();
                (Car::new(elevator), start)
            })
            .unzip();
        (Self { cars }, Task::batch(starts))
    }

    pub fn update(&mut self, message: Message, _building: &Building) -> Task<Message> {
        match message {
            Message::Elevator(elevator, message) => {
                let Some(car) = self.cars.iter_mut().find(|car| car.elevator == elevator) else {
                    return Task::none();
                };
                car.update(message)
            }
            Message::Floor(floor, _) => {
                self.cars[0].visit(floor);
                Task::none()
            }
        }
    }

    pub fn view(&self, building: &Building) -> impl Widget<Message> {
        elevato::view(building)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        elevato::riders()
    }
}

impl Car {
    fn new(elevator: Elevator) -> Self {
        Self {
            elevator,
            stops: VecDeque::new(),
        }
    }

    /// Adds a floor to visit, unless it is already the last one.
    fn visit(&mut self, floor: usize) {
        if self.stops.back() != Some(&floor) {
            self.stops.push_back(floor);
        }
    }

    fn update(&mut self, message: elevator::Message) -> Task<Message> {
        match message {
            elevator::Message::Idle => {
                let next = self.stops.pop_front().unwrap_or(0);
                self.elevator.go_to_floor(next)
            }
            elevator::Message::FloorButtonPressed(floor) => {
                self.visit(floor);
                Task::none()
            }
            elevator::Message::PassingFloor(..) | elevator::Message::StoppedAtFloor { .. } => {
                Task::none()
            }
        }
    }
}
