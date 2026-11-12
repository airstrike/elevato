//! Research §5.3-style ("Twentyliner"): every message re-plans every
//! car from scratch. Calls are the floors' lit buttons, kept as signed
//! levels the way the original kept them (down calls negative, so a
//! lobby up call, +0, has sign 0 and only a car with no direction takes
//! it - a faithful quirk). A car under half load considers the calls
//! along its direction; the nearest useful floor wins, the call is
//! claimed, and the car is sent there with its lamps set to match: its
//! one trip, replacing whatever trip it was on.

use elevato::{Building, Elevator, Message, elevator, floor};
use iced::{Element, Subscription, Task};

pub struct Program {
    cars: Vec<Car>,
    /// Lit call buttons: `+level` up, `-level` down.
    calls: Vec<i64>,
}

/// What the program knows of one car.
struct Car {
    elevator: Elevator,
    at: i64,
    /// Lit destination buttons inside.
    pressed: Vec<i64>,
    load_factor: f64,
    /// Where it was last sent, while it is on its way.
    target: Option<i64>,
    /// Its lamps: (up, down).
    lamps: (bool, bool),
}

impl Program {
    pub fn new(elevators: usize) -> (Self, Task<Message>) {
        let (cars, starts): (Vec<Car>, Vec<Task<Message>>) = (0..elevators)
            .map(|_| {
                let (elevator, start) = Elevator::new();
                let car = Car {
                    elevator,
                    at: 0,
                    pressed: Vec::new(),
                    load_factor: 0.0,
                    target: None,
                    lamps: (true, true),
                };
                (car, start)
            })
            .unzip();
        let program = Self {
            cars,
            calls: Vec::new(),
        };
        (program, Task::batch(starts))
    }

    pub fn update(&mut self, message: Message, _building: &Building) -> Task<Message> {
        match message {
            Message::Floor(floor, floor::Message::UpButtonPressed) => {
                self.calls.push(level(floor));
            }
            Message::Floor(floor, floor::Message::DownButtonPressed) => {
                self.calls.push(-level(floor));
            }
            Message::Elevator(elevator, message) => {
                let Some(car) = self.cars.iter_mut().find(|car| car.elevator == elevator) else {
                    return Task::none();
                };
                match message {
                    elevator::Message::Idle => car.target = None,
                    elevator::Message::FloorButtonPressed(floor) => car.pressed.push(level(floor)),
                    elevator::Message::PassingFloor(floor, _) => car.at = level(floor),
                    elevator::Message::StoppedAtFloor {
                        floor, load_factor, ..
                    } => {
                        let at = level(floor);
                        car.at = at;
                        car.load_factor = load_factor;
                        car.target = None;
                        car.pressed.retain(|&pressed| pressed != at);
                        // The car cleared the calls its lamps answered; a
                        // call of 0 is the lobby's up call.
                        let (up, down) = car.lamps;
                        self.calls.retain(|&call| {
                            !(call.abs() == at && ((call >= 0 && up) || (call < 0 && down)))
                        });
                    }
                }
            }
        }
        self.plan()
    }

    pub fn view(&self, building: &Building) -> Element<'_, Message> {
        elevato::view(building)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        elevato::riders()
    }

    fn plan(&mut self) -> Task<Message> {
        let mut calls = self.calls.clone();
        let mut tasks = Vec::new();
        for car in &mut self.cars {
            let at = car.at;
            // Direction: toward the last pressed floor that is not the
            // current one (the original's `.pop()`); 0 when none is.
            let mut direction = 0;
            for &pressed in &car.pressed {
                if pressed != at {
                    direction = (pressed - at).signum();
                }
            }

            // Pickups only when lightly loaded; a directed car takes
            // only same-direction calls at or beyond its position.
            let mut options = Vec::new();
            if car.load_factor < 0.5 {
                for &call in &calls {
                    let along = (call.abs() - at).signum();
                    if direction == 0
                        || (call.signum() == direction && (along == direction || along == 0))
                    {
                        options.push(call);
                    }
                }
            }
            options.extend(car.pressed.iter().copied());

            // The nearest useful floor wins; later entries win ties, like
            // the original's reduce.
            let distance = |call: i64| (call.abs() - at).abs();
            let mut best: Option<i64> = None;
            for &option in &options {
                if best.is_none_or(|best| distance(best) >= distance(option)) {
                    best = Some(option);
                }
            }

            let Some(best) = best else {
                continue;
            };
            calls.retain(|&call| call != best);
            let lamps = (direction >= 0, direction <= 0);
            if car.target == Some(best.abs()) && car.lamps == lamps {
                continue;
            }
            car.target = Some(best.abs());
            car.lamps = lamps;
            let elevator = car.elevator;
            tasks.push(Task::batch([
                elevator.set_going_down_indicator(lamps.1),
                elevator.set_going_up_indicator(lamps.0),
                elevator.go_to_floor(best.unsigned_abs() as usize),
            ]));
        }
        Task::batch(tasks)
    }
}

fn level(floor: usize) -> i64 {
    floor as i64
}
