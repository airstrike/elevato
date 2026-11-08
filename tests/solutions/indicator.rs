//! Research §5.2-style, "two sorted lists for up and down": the floors'
//! calls gather in two sorted lists, an idle car claims a whole list as
//! one sweep, riders' destinations join the car's stops in travel
//! order, and the indicators say where the car goes next, so only
//! riders headed that way get on.
//!
//! Each car is its own small iced component: [`Car`] owns its stops and
//! its `update` takes an [`elevator::Message`]; the program routes each
//! car's messages to it and maps its tasks back.

use elevato::{Direction, Elevator, elevator, floor};
use iced::{Function, Subscription, Task};

pub struct Program {
    cars: Vec<Car>,
    ups: Vec<usize>,
    downs: Vec<usize>,
}

#[derive(Debug, Clone)]
pub enum Message {
    Elevator(Elevator, elevator::Message),
    Floor(usize, floor::Message),
}

/// One car: where it is, the stops it will make in order, and how many
/// of them it has already been sent toward.
struct Car {
    elevator: Elevator,
    at: usize,
    stops: Vec<usize>,
    sent: usize,
}

impl Program {
    pub fn new(elevators: Vec<Elevator>, _floors: usize) -> (Self, Task<Message>) {
        let lobby = 0;
        let cars = elevators
            .into_iter()
            .map(|elevator| Car::new(elevator, lobby))
            .collect();
        let program = Self {
            cars,
            ups: Vec::new(),
            downs: Vec::new(),
        };
        (program, Task::none())
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Floor(floor, floor::Message::UpButtonPressed) => {
                insert_sorted(&mut self.ups, floor);
                self.claim()
            }
            Message::Floor(floor, floor::Message::DownButtonPressed) => {
                insert_sorted(&mut self.downs, floor);
                self.claim()
            }
            Message::Elevator(elevator, message) => {
                let Some(car) = self.cars.iter_mut().find(|car| car.elevator == elevator) else {
                    return Task::none();
                };
                let task = car.update(message).map(Message::Elevator.with(elevator));
                Task::batch([task, self.claim()])
            }
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        elevato::riders(Message::Elevator, Message::Floor)
    }

    /// Hands the waiting calls to the first idle car: the up calls as
    /// one ascending sweep, else the down calls as one descending sweep.
    /// Riders a full car leaves behind press again, filling the lists
    /// for the next idle car.
    fn claim(&mut self) -> Task<Message> {
        let Some(car) = self.cars.iter_mut().find(|car| car.stops.is_empty()) else {
            return Task::none();
        };
        let sweep: Vec<usize> = if !self.ups.is_empty() {
            std::mem::take(&mut self.ups)
        } else if !self.downs.is_empty() {
            std::mem::take(&mut self.downs).into_iter().rev().collect()
        } else {
            return Task::none();
        };
        let elevator = car.elevator;
        car.sweep(sweep).map(Message::Elevator.with(elevator))
    }
}

impl Car {
    fn new(elevator: Elevator, at: usize) -> Self {
        Self {
            elevator,
            at,
            stops: Vec::new(),
            sent: 0,
        }
    }

    fn update(&mut self, message: elevator::Message) -> Task<elevator::Message> {
        match message {
            elevator::Message::FloorButtonPressed(floor) => self.drop_off(floor),
            elevator::Message::PassingFloor(floor, _) => {
                self.at = floor;
                Task::none()
            }
            elevator::Message::StoppedAtFloor { floor, .. } => {
                self.at = floor;
                if let Some(index) = self.stops.iter().position(|&stop| stop == floor) {
                    let _ = self.stops.remove(index);
                    self.sent -= 1;
                }
                self.next()
            }
        }
    }

    /// Takes a sweep of pickups, in the order given.
    fn sweep(&mut self, floors: Vec<usize>) -> Task<elevator::Message> {
        for floor in floors {
            if !self.stops.contains(&floor) {
                self.stops.push(floor);
            }
        }
        self.next()
    }

    /// A rider's destination joins the stops in travel order, so the car
    /// sweeps one way instead of ping-ponging in press order.
    fn drop_off(&mut self, floor: usize) -> Task<elevator::Message> {
        if self.stops.contains(&floor) {
            return Task::none();
        }
        let between = |a: usize, b: usize| (a <= floor && floor <= b) || (b <= floor && floor <= a);
        match self.stops.first() {
            Some(&next) if self.sent > 0 && between(self.at, next) => {
                // On the way to the stop it was sent to: stop here first.
                self.stops.insert(0, floor);
                self.sent += 1;
                Task::batch([self.indicate(), self.elevator.go_to_floor_first(floor)])
            }
            _ => {
                let mut from = self.stops.first().copied().unwrap_or(self.at);
                let mut index = self.stops.len();
                for (position, &stop) in self.stops.iter().enumerate().skip(1) {
                    if between(from, stop) {
                        index = position;
                        break;
                    }
                    from = stop;
                }
                self.stops.insert(index, floor);
                self.next()
            }
        }
    }

    /// Sends the car toward its first stop when it is not on its way to
    /// one, lamps lit for the way it goes from there.
    fn next(&mut self) -> Task<elevator::Message> {
        if self.sent > 0 {
            return self.indicate();
        }
        let Some(&stop) = self.stops.first() else {
            return Task::batch([
                self.elevator.set_going_up_indicator(true),
                self.elevator.set_going_down_indicator(true),
            ]);
        };
        self.sent = 1;
        Task::batch([self.indicate(), self.elevator.go_to_floor(stop)])
    }

    /// Lights the lamps for the direction the car leaves its first stop
    /// in: both when that stop is its last.
    fn indicate(&self) -> Task<elevator::Message> {
        let direction = match self.stops.as_slice() {
            [first, second, ..] if second > first => Some(Direction::Up),
            [first, second, ..] if second < first => Some(Direction::Down),
            _ => None,
        };
        Task::batch([
            self.elevator
                .set_going_up_indicator(direction != Some(Direction::Down)),
            self.elevator
                .set_going_down_indicator(direction != Some(Direction::Up)),
        ])
    }
}

/// Adds `floor` to a sorted list of calls, once.
fn insert_sorted(calls: &mut Vec<usize>, floor: usize) {
    if let Err(index) = calls.binary_search(&floor) {
        calls.insert(index, floor);
    }
}
