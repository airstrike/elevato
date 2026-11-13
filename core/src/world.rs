//! The simulated world: floors, elevators, passengers, stats, the event
//! queue, and the `step`/drain/`process_arrivals` contract the driver
//! runs.
//!
//! # Driver contract
//!
//! Drivers dispatch events before and after each substep's arrival
//! processing, so handlers run when the original's synchronous events
//! fired. Both [`crate::headless::run`] and `elevato_api::simulation`
//! follow this contract:
//!
//! ```text
//! while remaining > 0.0 && !world.ended() {
//!     world.step(DT_MAX.min(remaining));
//!     for event in world.drain_events() { /* dispatch */ }
//!     world.process_arrivals();
//!     for event in world.drain_events() { /* dispatch */ }
//!     remaining -= DT_MAX;
//! }
//! ```
//!
//! Commands ([`World::go_to_floor`] etc.) are called between drains; any
//! events they raise (e.g. an immediate `Idle`) land in the queue for the
//! next drain.
//!
//! # Cars
//!
//! A world starts with no cars. [`World::add_elevator`] adds the next car
//! of the challenge's bank, standing idle at the lobby, until the bank is
//! used up. Each car makes one trip at a time: a new trip replaces the
//! current one.
//!
//! # Step order
//!
//! [`World::step`] advances time, spawns passengers, moves elevators, and
//! advances passenger walk timers. Wait times include passengers whose
//! exit walk completes in this step. Challenge conditions are evaluated
//! at the end of [`World::process_arrivals`], so this substep's exits count.
//!
//! # Staged arrivals
//!
//! Reaching a trip's destination ends the trip and starts the 1 s dwell
//! inside the step, as the original's `stopped` fires before
//! `stopped_at_floor`. Core `StoppedAtFloor` handlers therefore see a busy
//! elevator with no trip. Boarding waits for [`World::process_arrivals`],
//! so handlers can set the indicators before riders get on.
//!
//! # Passing-floor dispatch latency
//!
//! `PassingFloor` is emitted during a step but dispatched at the next
//! drain, one substep after detection. That delay is tolerated by design:
//! the 5% braking-engage margin at top speed (~3.25 px) exceeds one
//! substep's drift (~2.2 px), so a `go_to_floor(floor)` issued from the
//! handler still stops in time.

use crate::challenge::{Challenge, Condition, Outcome};
use crate::elevator::Elevator;
use crate::event::{Direction, Event};
use crate::floor::Floor;
use crate::passenger;
use crate::passenger::{Appearance, Passenger};
use crate::rng::Pcg32;
use crate::stats::Stats;

/// Longest physics substep, in seconds. [`World::step`] clamps to this;
/// drivers wanting more simulated time call it repeatedly.
pub const DT_MAX: f64 = 1.0 / 60.0;

/// A deterministic world built from a validated [`Challenge`] and a seed.
/// The challenge supplies valid floor counts, capacities, and spawn rate.
#[derive(Debug, Clone)]
pub struct World {
    floors: Vec<Floor>,
    elevators: Vec<Elevator>,
    /// The capacity of each car the challenge's bank can still add, in
    /// order.
    bank: Vec<usize>,
    passengers: Vec<Passenger>,
    events: Vec<Event>,
    pending_arrivals: Vec<usize>,
    rng: Pcg32,
    spawn_rate: f64,
    /// Simulated seconds toward the next spawn. For a positive spawn
    /// rate, starts at `1.001 / spawn_rate` so the first step spawns a rider.
    elapsed_since_spawn: f64,
    condition: Condition,
    outcome: Outcome,
    stats: Stats,
}

impl World {
    /// Starts with empty floors and a seeded spawn stream.
    /// [`World::add_elevator`] draws cars from the challenge's capacity
    /// cycle; no cars are present until it is called.
    pub fn new(challenge: &Challenge, seed: u64) -> Self {
        let floor_count = challenge.floor_count();
        let spawn_rate = challenge.spawn_rate();
        Self {
            floors: (0..floor_count)
                .map(|level| Floor::new(level, floor_count))
                .collect(),
            elevators: Vec::new(),
            bank: (0..challenge.elevator_count())
                .map(|id| challenge.capacity(id))
                .collect(),
            passengers: Vec::new(),
            events: Vec::new(),
            pending_arrivals: Vec::new(),
            rng: Pcg32::new(seed),
            spawn_rate,
            elapsed_since_spawn: if spawn_rate > 0.0 {
                1.001 / spawn_rate
            } else {
                0.0
            },
            condition: challenge.condition(),
            outcome: Outcome::Running,
            stats: Stats::default(),
        }
    }

    /// The building's floors, ground first.
    pub fn floors(&self) -> &[Floor] {
        &self.floors
    }

    /// The elevators; event `elevator` fields index into this slice.
    pub fn elevators(&self) -> &[Elevator] {
        &self.elevators
    }

    /// Mutable access for indicator changes. Commands that raise events
    /// go through the world. Panics if `elevator` is outside this world's
    /// elevator list.
    pub fn elevator_mut(&mut self, elevator: usize) -> &mut Elevator {
        self.elevators
            .get_mut(elevator)
            .expect("invariant: elevator index from this world")
    }

    /// Everyone currently present, in spawn order: waiting, aboard, and
    /// walking off after exiting.
    pub fn passengers(&self) -> &[Passenger] {
        &self.passengers
    }

    /// The live statistics snapshot.
    pub fn stats(&self) -> &Stats {
        &self.stats
    }

    /// Simulated seconds advanced so far.
    pub fn elapsed(&self) -> f64 {
        self.stats.elapsed()
    }

    /// How the challenge stands; flips the moment its condition decides.
    pub fn outcome(&self) -> Outcome {
        self.outcome
    }

    /// Whether the challenge has ended. Flips mid-frame; the driver loop
    /// breaks on it before the next substep.
    pub fn ended(&self) -> bool {
        self.outcome != Outcome::Running
    }

    /// Advances by `dt` seconds, clamped between zero and [`DT_MAX`].
    /// Does nothing once the challenge ends. See the module docs for the
    /// update order and the driver's arrival-processing contract.
    pub fn step(&mut self, dt: f64) {
        if self.ended() {
            return;
        }
        let dt = dt.clamp(0.0, DT_MAX);
        self.stats.advance(dt);
        self.spawn_due_passengers(dt);
        for elevator in &mut self.elevators {
            elevator.update_tasks(dt, &mut self.events);
            elevator.update_movement(dt, &mut self.events, &mut self.pending_arrivals);
        }
        self.tick_passengers(dt);
        self.stats
            .set_move_count(self.elevators.iter().map(Elevator::move_count).sum());
    }

    /// Takes every event raised since the last drain, oldest first.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Processes staged arrivals after the driver dispatches core
    /// `StoppedAtFloor` events. The call buttons matching the car's lit
    /// indicators clear before boarding, so riders left behind can press
    /// again. Exits free slots before riders board in spawn order. The
    /// challenge condition is evaluated last, so a deciding exit ends the
    /// challenge in this substep.
    pub fn process_arrivals(&mut self) {
        if self.ended() {
            self.pending_arrivals.clear();
            return;
        }
        let arrivals = std::mem::take(&mut self.pending_arrivals);
        for elevator in arrivals {
            self.process_arrival(elevator);
        }
        if let Some(success) = self.condition.evaluate(&self.stats) {
            self.outcome = if success {
                Outcome::Succeeded
            } else {
                Outcome::Failed
            };
        }
    }

    /// How many cars the challenge's bank holds, added or not.
    pub fn bank_size(&self) -> usize {
        self.elevators.len() + self.bank.len()
    }

    /// Adds the bank's next car, standing idle at the lobby, and raises
    /// its [`Event::Idle`]. `None` once every car of the bank stands in
    /// the world.
    pub fn add_elevator(&mut self) -> Option<usize> {
        if self.bank.is_empty() {
            return None;
        }
        let capacity = self.bank.remove(0);
        let id = self.elevators.len();
        self.elevators
            .push(Elevator::new(id, self.floors.len(), capacity));
        self.events.push(Event::Idle { elevator: id });
        Some(id)
    }

    /// Starts a trip, replacing the current one; see [`Elevator::destination`].
    /// Panics if `elevator` is outside this world's elevator list.
    pub fn go_to_floor(&mut self, elevator: usize, floor: f64) {
        let elevator = self
            .elevators
            .get_mut(elevator)
            .expect("invariant: elevator index from this world");
        elevator.go_to_floor(floor, &mut self.events);
    }

    /// Ends a trip and, unless the car is dwelling, halts it at the
    /// projected stop point. A halt between floors emits no arrival and
    /// starts no dwell. Panics if `elevator` is outside the elevator list.
    pub fn stop(&mut self, elevator: usize) {
        let elevator = self
            .elevators
            .get_mut(elevator)
            .expect("invariant: elevator index from this world");
        elevator.stop();
    }

    /// Reopens a standing car ahead of its trip so a rider who presses
    /// the call beside it can board. Panics if `elevator` is outside this
    /// world's elevator list.
    pub fn reopen(&mut self, elevator: usize) {
        let elevator = self
            .elevators
            .get_mut(elevator)
            .expect("invariant: elevator index from this world");
        elevator.reopen(&mut self.events);
    }

    /// Presses a destination button, emitting [`Event::FloorButtonPressed`]
    /// only when it is unlit. Passengers press after their 1 s boarding walk.
    /// Panics if `elevator` or `floor` is outside this world's lists.
    pub fn press_floor_button(&mut self, elevator: usize, floor: usize) {
        let elevator = self
            .elevators
            .get_mut(elevator)
            .expect("invariant: elevator index from this world");
        elevator.press_floor_button(floor, &mut self.events);
    }

    /// Spawns every passenger due within this substep: one immediately on
    /// the first step (the accumulator starts past the interval), then
    /// exactly one per `1 / spawn_rate` game-seconds, several per substep
    /// when the interval is shorter than `dt`.
    fn spawn_due_passengers(&mut self, dt: f64) {
        if self.spawn_rate <= 0.0 {
            return;
        }
        self.elapsed_since_spawn += dt;
        let interval = 1.0 / self.spawn_rate;
        while self.elapsed_since_spawn > interval {
            self.elapsed_since_spawn -= interval;
            self.spawn_passenger();
        }
    }

    /// Draws weight, appearance, spawn floor, then destination in a fixed
    /// order for replay. The passenger immediately presses a call button.
    fn spawn_passenger(&mut self) {
        let weight = self.rng.random_inclusive(55, 100);
        let appearance = if self.rng.random_inclusive(0, 40) == 0 {
            Appearance::Child
        } else if self.rng.random_inclusive(0, 1) == 0 {
            Appearance::Female
        } else {
            Appearance::Male
        };
        let top = (self.floors.len() - 1) as u32;
        let spawn_floor = if self.rng.random_inclusive(0, 1) == 0 {
            0
        } else {
            self.rng.random_inclusive(0, top) as usize
        };
        let destination = if spawn_floor == 0 {
            // From the lobby: uniformly up.
            self.rng.random_inclusive(1, top) as usize
        } else if self.rng.random_inclusive(0, 10) == 0 {
            // 1-in-11: a uniform *other* floor.
            (spawn_floor + self.rng.random_inclusive(1, top) as usize) % self.floors.len()
        } else {
            // Otherwise the lobby (~91% of upper-floor spawns).
            0
        };
        self.passengers.push(Passenger::new(
            weight,
            appearance,
            spawn_floor,
            destination,
            self.stats.elapsed(),
        ));
        let direction = if destination < spawn_floor {
            Direction::Down
        } else {
            Direction::Up
        };
        self.press_call_button(spawn_floor, direction);
    }

    /// Lights a floor call button. Only the unlit → lit transition emits
    /// the event and runs the re-arrival scan - pressing a lit button is
    /// silent, exactly like the original.
    fn press_call_button(&mut self, level: usize, direction: Direction) {
        let transitioned = self.floors[level].press(direction);
        if !transitioned {
            return;
        }
        self.events.push(match direction {
            Direction::Up => Event::UpButtonPressed { floor: level },
            Direction::Down => Event::DownButtonPressed { floor: level },
        });
        self.dispatch_standing_elevator(level, direction);
    }

    /// The original's `handleButtonRepressing`: scans from a random
    /// rotation for a car standing still exactly on this floor, not full,
    /// with the matching indicator lit, and reopens it so the caller can
    /// board. One offset is drawn per scan.
    fn dispatch_standing_elevator(&mut self, level: usize, direction: Direction) {
        let count = self.elevators.len();
        let offset = self.rng.random_inclusive(0, (count - 1) as u32) as usize;
        for scan in 0..count {
            let id = (scan + offset) % count;
            let elevator = &self.elevators[id];
            let indicator = match direction {
                Direction::Up => elevator.going_up_indicator(),
                Direction::Down => elevator.going_down_indicator(),
            };
            if indicator
                && elevator.current_floor() == level
                && elevator.is_on_a_floor()
                && !elevator.is_moving()
                && !elevator.is_full()
            {
                self.reopen(id);
                return;
            }
        }
    }

    /// Walk timers, the every-step `max_wait_time` refresh (still fed by
    /// passengers walking off after exiting), and removals.
    fn tick_passengers(&mut self, dt: f64) {
        let mut removals = Vec::new();
        for (index, passenger) in self.passengers.iter_mut().enumerate() {
            match passenger.tick(dt) {
                Some(passenger::Completion::BoardWalk {
                    elevator,
                    destination,
                }) => {
                    self.elevators[elevator].press_floor_button(destination, &mut self.events);
                }
                Some(passenger::Completion::ExitWalk) => removals.push(index),
                None => {}
            }
            let wait = self.stats.elapsed() - passenger.spawn_time();
            self.stats.observe_wait(wait);
        }
        // Reverse order keeps the still-pending indices valid.
        for index in removals.into_iter().rev() {
            self.passengers.remove(index);
        }
    }

    /// Clears calls and frees exiting riders' slots before boarding.
    fn process_arrival(&mut self, elevator: usize) {
        let level = self.elevators[elevator].current_floor();

        // Clear the calls matching a lit indicator before boarding, so
        // riders left behind can press again and raise a fresh event.
        if self.elevators[elevator].going_up_indicator() {
            self.floors[level].clear(Direction::Up);
        }
        if self.elevators[elevator].going_down_indicator() {
            self.floors[level].clear(Direction::Down);
        }

        // Exits free slots before boarding. The exit records wait time,
        // but the walk-off still counts toward max_wait_time until removal.
        for index in 0..self.passengers.len() {
            let passenger = &self.passengers[index];
            if passenger.aboard() != Some(elevator) || passenger.destination_floor() != level {
                continue;
            }
            let slot = passenger
                .slot()
                .expect("invariant: aboard passengers occupy a slot");
            self.elevators[elevator].free_slot(slot);
            let wait = self.stats.elapsed() - passenger.spawn_time();
            self.stats.record_exit(wait);
            let walk = 1.0 + self.rng.random_f64() * 0.5;
            self.passengers[index].exit(level, walk);
        }

        // Every suitable boarding attempt draws a slot offset, even for
        // a full car, to keep the random stream stable across replays.
        for index in 0..self.passengers.len() {
            let passenger = &self.passengers[index];
            if !passenger.is_waiting() || passenger.current_floor() != level {
                continue;
            }
            let destination = passenger.destination_floor();
            if !self.elevators[elevator].is_suitable_for_travel_between(level, destination) {
                continue;
            }
            let weight = passenger.weight();
            let capacity = self.elevators[elevator].capacity();
            let offset = self.rng.random_inclusive(0, (capacity - 1) as u32) as usize;
            match self.elevators[elevator].enter_slot(weight, offset) {
                Some(slot) => self.passengers[index].board(elevator, slot),
                None => {
                    // No free slot: the passenger stays and re-presses
                    // the just-cleared call button, re-firing the event
                    // and re-running the re-arrival scan.
                    let direction = if destination < level {
                        Direction::Down
                    } else {
                        Direction::Up
                    };
                    self.press_call_button(level, direction);
                }
            }
        }
    }
}
