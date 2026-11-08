//! Research §5.1, the wiki's "easy solution": every car serves its own
//! riders' buttons, goes back to the lobby once it has nothing to do,
//! and every floor's call goes to the first car. No direction or load
//! awareness: it clears the first few challenges only.
//!
//! Each car is its own small iced component: [`Car`] owns its state,
//! its `update` takes an [`elevator::Message`], and the program routes
//! each car's messages to it and maps its tasks back.

use elevato::{Elevator, Rider, elevator, floor};
use iced::{Function, Subscription, Task};

pub struct Program {
    cars: Vec<Car>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Elevator(Elevator, elevator::Message),
    Floor(usize, floor::Message),
}

/// One car and the trips it has under way.
struct Car {
    elevator: Elevator,
    trips: usize,
}

impl Program {
    pub fn new(elevators: Vec<Elevator>, _floors: usize) -> (Self, Task<Message>) {
        let mut cars: Vec<Car> = elevators.into_iter().map(Car::new).collect();
        let start = Task::batch(cars.iter_mut().map(|car| {
            let elevator = car.elevator;
            car.go_to_floor(0).map(Message::Elevator.with(elevator))
        }));
        (Self { cars }, start)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Elevator(elevator, message) => {
                let Some(car) = self.cars.iter_mut().find(|car| car.elevator == elevator) else {
                    return Task::none();
                };
                car.update(message).map(Message::Elevator.with(elevator))
            }
            Message::Floor(floor, _) => {
                let car = &mut self.cars[0];
                let elevator = car.elevator;
                car.go_to_floor(floor).map(Message::Elevator.with(elevator))
            }
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        elevato::riders().map(|rider| match rider {
            Rider::Elevator(elevator, message) => Message::Elevator(elevator, message),
            Rider::Floor(floor, message) => Message::Floor(floor, message),
        })
    }
}

impl Car {
    fn new(elevator: Elevator) -> Self {
        Self { elevator, trips: 0 }
    }

    fn go_to_floor(&mut self, floor: usize) -> Task<elevator::Message> {
        self.trips += 1;
        self.elevator.go_to_floor(floor)
    }

    fn update(&mut self, message: elevator::Message) -> Task<elevator::Message> {
        match message {
            elevator::Message::FloorButtonPressed(floor) => self.go_to_floor(floor),
            elevator::Message::PassingFloor(..) => Task::none(),
            elevator::Message::StoppedAtFloor { .. } => {
                self.trips -= 1;
                if self.trips == 0 {
                    self.go_to_floor(0)
                } else {
                    Task::none()
                }
            }
        }
    }
}
