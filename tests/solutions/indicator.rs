//! Research §5.2-style, "two sorted lists for up and down": the floors'
//! calls gather in two sorted lists, an idle car claims a whole list as
//! one sweep, riders' destinations join the car's stops in travel
//! order, and the indicators say where the car goes next, so only
//! riders headed that way get on.
//!
//! Each car is its own small iced component: [`Car`] owns its stops and
//! its `update` takes an [`elevator::Message`]; the program routes each
//! car's messages to it and maps its tasks back. A car makes one trip at
//! a time, to its first stop; a stop it should make on the way replaces
//! that trip.

use elevato::{Building, Direction, Elevator, Message, elevator, floor};
use iced::{Element, Subscription, Task};

pub struct Program {
    cars: Vec<Car>,
    ups: Vec<usize>,
    downs: Vec<usize>,
}

/// One car: where it is, the stops it will make in order, and the stop
/// it is on its way to.
struct Car {
    elevator: Elevator,
    at: usize,
    stops: Vec<usize>,
    heading: Option<usize>,
}

impl Program {
    pub fn new(elevators: usize) -> (Self, Task<Message>) {
        let (cars, starts): (Vec<Car>, Vec<Task<Message>>) = (0..elevators)
            .map(|_| {
                let (elevator, start) = Elevator::new();
                (Car::new(elevator), start)
            })
            .unzip();
        let program = Self {
            cars,
            ups: Vec::new(),
            downs: Vec::new(),
        };
        (program, Task::batch(starts))
    }

    pub fn update(&mut self, message: Message, _building: &Building) -> Task<Message> {
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
                let task = car.update(message);
                Task::batch([task, self.claim()])
            }
        }
    }

    pub fn view(&self, building: &Building) -> Element<'_, Message> {
        elevato::view(building)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        elevato::riders()
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
        car.sweep(sweep)
    }
}

impl Car {
    fn new(elevator: Elevator) -> Self {
        Self {
            elevator,
            at: 0,
            stops: Vec::new(),
            heading: None,
        }
    }

    fn update(&mut self, message: elevator::Message) -> Task<Message> {
        match message {
            elevator::Message::Idle => {
                self.heading = None;
                self.next()
            }
            elevator::Message::FloorButtonPressed(floor) => self.drop_off(floor),
            elevator::Message::PassingFloor(floor, _) => {
                self.at = floor;
                Task::none()
            }
            elevator::Message::StoppedAtFloor { floor, .. } => {
                self.at = floor;
                if self.heading == Some(floor) {
                    self.heading = None;
                    self.stops.retain(|&stop| stop != floor);
                }
                self.next()
            }
        }
    }

    /// Takes a sweep of pickups, in the order given.
    fn sweep(&mut self, floors: Vec<usize>) -> Task<Message> {
        for floor in floors {
            if !self.stops.contains(&floor) {
                self.stops.push(floor);
            }
        }
        self.next()
    }

    /// A rider's destination joins the stops in travel order, so the car
    /// sweeps one way instead of ping-ponging in press order.
    fn drop_off(&mut self, floor: usize) -> Task<Message> {
        if self.stops.contains(&floor) {
            return Task::none();
        }
        let between = |a: usize, b: usize| (a <= floor && floor <= b) || (b <= floor && floor <= a);
        match self.heading {
            Some(next) if between(self.at, next) => {
                // On the way to the stop it was sent to: stop here first.
                self.stops.insert(0, floor);
                self.heading = Some(floor);
                Task::batch([self.indicate(), self.elevator.go_to_floor(floor)])
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
    fn next(&mut self) -> Task<Message> {
        if self.heading.is_some() {
            return self.indicate();
        }
        let Some(&stop) = self.stops.first() else {
            return Task::batch([
                self.elevator.set_going_up_indicator(true),
                self.elevator.set_going_down_indicator(true),
            ]);
        };
        self.heading = Some(stop);
        Task::batch([self.indicate(), self.elevator.go_to_floor(stop)])
    }

    /// Lights the lamps for the direction the car leaves its first stop
    /// in: both when that stop is its last.
    fn indicate(&self) -> Task<Message> {
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
