//! The simulated world: floors, elevators, passengers, stats, the event
//! queue, and the `step`/drain/`process_arrivals` contract the driver
//! runs.
//!
//! # Driver contract
//!
//! The substep loop is **not** in core — [`crate::headless::run`] owns it
//! (and is the only driver), so handlers dispatch per-substep exactly as
//! the original's synchronous events did:
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
//! # Step order
//!
//! [`World::step`] mirrors the original `world.update`: advance elapsed →
//! spawn due passengers → elevator task + movement updates (arrivals are
//! staged) → passenger walk timers + the every-step `max_wait_time`
//! refresh → removals → move-count refresh. Challenge conditions are
//! evaluated at the end of [`World::process_arrivals`] — after this
//! substep's exits landed — matching the original's `stats_changed`
//! evaluation running after its (inline) arrival processing.
//!
//! # Staged arrivals
//!
//! When a step lands an elevator exactly on a floor, the queue-head shift
//! and the 1 s dwell start happen synchronously inside the step (the
//! original's `stopped` reaction runs *before* `stopped_at_floor` fires),
//! so `StoppedAtFloor` handlers observe a busy, already-shifted elevator —
//! but the arrival stays staged until [`World::process_arrivals`], which
//! clears matching floor call buttons, lets riders exit, and boards
//! waiting passengers, strictly after `StoppedAtFloor` handlers ran.
//!
//! # Passing-floor dispatch latency
//!
//! `PassingFloor` is emitted during a step but dispatched at the next
//! drain, one substep after detection. That delay is tolerated by design:
//! the 5% braking-engage margin at top speed (~3.25 px) exceeds one
//! substep's drift (~2.2 px), so a `go_to_floor(floor, force)` issued from
//! the handler still stops in time.

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

/// A deterministic, headless elevator world, minted from a validated
/// [`Challenge`] and a seed (`transformation-method`: the challenge is
/// the proof bundle, so construction cannot fail).
#[derive(Debug)]
pub struct World {
    floors: Vec<Floor>,
    elevators: Vec<Elevator>,
    passengers: Vec<Passenger>,
    events: Vec<Event>,
    pending_arrivals: Vec<usize>,
    rng: Pcg32,
    spawn_rate: f64,
    /// Game-seconds accumulated toward the next spawn. Starts at
    /// `1.001 / spawn_rate` so one passenger spawns on the very first
    /// step (research §3).
    elapsed_since_spawn: f64,
    condition: Condition,
    outcome: Outcome,
    stats: Stats,
}

impl World {
    /// Mints a world from a challenge: floors, elevators standing at
    /// floor 0 with capacities cycling `capacities[i % len]`, the seeded
    /// spawn stream, and the challenge's condition ready to evaluate.
    pub fn new(challenge: &Challenge, seed: u64) -> Self {
        let floor_count = challenge.floor_count();
        let spawn_rate = challenge.spawn_rate();
        Self {
            floors: (0..floor_count)
                .map(|level| Floor::new(level, floor_count))
                .collect(),
            elevators: (0..challenge.elevator_count())
                .map(|id| Elevator::new(id, floor_count, challenge.capacity(id)))
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

    /// Mutable access to one elevator, for the command-free mutations
    /// (indicator setters). Event-raising commands live on the world.
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

    /// Advances the simulation by `dt` seconds, clamped to `[0,`
    /// [`DT_MAX`]`]`, in the original's update order (see the module
    /// docs). Does nothing once the challenge has ended.
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

    /// Processes arrivals staged by [`World::step`], after the driver has
    /// dispatched their `StoppedAtFloor` events. Per arrived elevator, in
    /// research §3 order: clear the floor call button(s) matching the
    /// elevator's lit indicator(s) — first, deliberately, so overflow
    /// passengers re-press — then exits (freeing slots), then boarding in
    /// spawn order. Ends by evaluating the challenge condition, so a
    /// decision lands the same substep as the exit that caused it.
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

    /// Queues a destination for an elevator; see
    /// [`Elevator::destination_queue`] for the resulting queue. `force`
    /// puts it at the front of the queue.
    pub fn go_to_floor(&mut self, elevator: usize, floor: f64, force: bool) {
        let elevator = self
            .elevators
            .get_mut(elevator)
            .expect("invariant: elevator index from this world");
        elevator.go_to_floor(floor, force, &mut self.events);
    }

    /// Clears an elevator's queue and, unless it is dwelling, halts it at
    /// the projected stop point (generally between floors — no arrival
    /// events, no dwell).
    pub fn stop(&mut self, elevator: usize) {
        let elevator = self
            .elevators
            .get_mut(elevator)
            .expect("invariant: elevator index from this world");
        elevator.stop();
    }

    /// Re-checks an elevator's destination queue: starts movement toward
    /// the front, or emits [`Event::Idle`] if the queue is empty (and the
    /// elevator is not busy). The driver calls this for every elevator at
    /// challenge start to fire the initial idle round.
    pub fn check_destination_queue(&mut self, elevator: usize) {
        let elevator = self
            .elevators
            .get_mut(elevator)
            .expect("invariant: elevator index from this world");
        elevator.check_destination_queue(&mut self.events);
    }

    /// Presses an in-elevator destination button (passengers do this
    /// 1.0 s after boarding, when their walk to the slot completes),
    /// emitting [`Event::FloorButtonPressed`] on the unlit → lit
    /// transition.
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

    /// One spawn, consuming randomness in the documented order: weight →
    /// display type → spawn floor → destination (research §3/§6). The
    /// newcomer immediately presses the floor's call button.
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
    /// the event and runs the re-arrival scan — pressing a lit button is
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

    /// The re-arrival rule (original `handleButtonRepressing`): scan the
    /// elevators in a random-rotation order for one standing still on
    /// this exact floor, not full, with the matching direction indicator
    /// on — and issue it a forced `go_to_floor`, causing a re-arrival the
    /// presser can board. The rotation offset is drawn once per scan.
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
                self.elevators[id].go_to_floor(level as f64, true, &mut self.events);
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

    /// One arrived elevator's button-clear → exits → boarding sequence
    /// (research §3, in exactly that order).
    fn process_arrival(&mut self, elevator: usize) {
        let level = self.elevators[elevator].current_floor();

        // 1. Clear the call button(s) whose direction matches a lit
        //    indicator — before boarding, so overflow passengers re-press
        //    and the press event re-fires.
        if self.elevators[elevator].going_up_indicator() {
            self.floors[level].clear(Direction::Up);
        }
        if self.elevators[elevator].going_down_indicator() {
            self.floors[level].clear(Direction::Down);
        }

        // 2. Exits: every rider bound here leaves, freeing its slot ahead
        //    of boarding. Wait time is recorded into the stats now; the
        //    walk-off keeps feeding max_wait_time until removal.
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

        // 3. Boarding: every waiting passenger on this floor, in spawn
        //    order. Suitability checks the indicators; capacity is slot
        //    count only. The slot offset is drawn per suitable attempt —
        //    even when the elevator turns out to be full — to keep the
        //    RNG stream aligned with the original's `userEntering`.
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
