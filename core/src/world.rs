//! The simulated world: floors, elevators, the event queue, and the
//! `step`/drain/`process_arrivals` contract the driver runs.
//!
//! # Driver contract
//!
//! The substep loop is **not** in core — drivers (the script runtime, the
//! headless runner, tests) own it, so handlers dispatch per-substep exactly
//! as the original's synchronous events did:
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
//! # Staged arrivals
//!
//! When a step lands an elevator exactly on a floor, the queue-head shift
//! and the 1 s dwell start happen synchronously inside the step (the
//! original's `stopped` reaction runs *before* `stopped_at_floor` fires),
//! so `StoppedAtFloor` handlers observe a busy, already-shifted elevator —
//! but the arrival stays staged until [`World::process_arrivals`], which in
//! Phase 3 clears matching floor call buttons, lets riders exit, and
//! boards waiting passengers, strictly after `StoppedAtFloor` handlers ran.
//!
//! # Passing-floor dispatch latency
//!
//! `PassingFloor` is emitted during a step but dispatched at the next
//! drain, one substep after detection. That delay is tolerated by design:
//! the 5% braking-engage margin at top speed (~3.25 px) exceeds one
//! substep's drift (~2.2 px), so a `go_to_floor(floor, force)` issued from
//! the handler still stops in time.

use crate::elevator::Elevator;
use crate::event::Event;
use crate::floor::Floor;

/// Longest physics substep, in seconds. [`World::step`] clamps to this;
/// drivers wanting more simulated time call it repeatedly.
pub const DT_MAX: f64 = 1.0 / 60.0;

/// Why a world could not be built.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The requested floor count was zero.
    #[error("a world needs at least one floor")]
    NoFloors,
    /// The requested elevator count was zero.
    #[error("a world needs at least one elevator")]
    NoElevators,
}

/// A deterministic, headless elevator world. Minted from validated counts;
/// passengers, challenges, and the seeded spawn stream arrive in Phase 3.
#[derive(Debug)]
pub struct World {
    floors: Vec<Floor>,
    elevators: Vec<Elevator>,
    events: Vec<Event>,
    pending_arrivals: Vec<usize>,
    elapsed: f64,
}

impl World {
    /// Builds a world with `floor_count` floors and `elevator_count`
    /// elevators, all standing at floor 0.
    pub fn new(floor_count: usize, elevator_count: usize) -> Result<Self, Error> {
        if floor_count == 0 {
            return Err(Error::NoFloors);
        }
        if elevator_count == 0 {
            return Err(Error::NoElevators);
        }
        Ok(Self {
            floors: (0..floor_count)
                .map(|level| Floor::new(level, floor_count))
                .collect(),
            elevators: (0..elevator_count)
                .map(|id| Elevator::new(id, floor_count))
                .collect(),
            events: Vec::new(),
            pending_arrivals: Vec::new(),
            elapsed: 0.0,
        })
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

    /// Simulated seconds advanced so far.
    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }

    /// Whether the challenge has ended. Always `false` until challenges
    /// land in Phase 3; the driver loop should already consult it.
    pub fn ended(&self) -> bool {
        false
    }

    /// Advances the simulation by `dt` seconds, clamped to `[0,`
    /// [`DT_MAX`]`]`. Task timers (door dwell) tick before physics, so a
    /// dwell that completes here can start the next trip within the same
    /// substep.
    pub fn step(&mut self, dt: f64) {
        let dt = dt.clamp(0.0, DT_MAX);
        self.elapsed += dt;
        for elevator in &mut self.elevators {
            elevator.update_tasks(dt, &mut self.events);
            elevator.update_movement(dt, &mut self.events, &mut self.pending_arrivals);
        }
    }

    /// Takes every event raised since the last drain, oldest first.
    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    /// Processes arrivals staged by [`World::step`], after the driver has
    /// dispatched their `StoppedAtFloor` events. Phase 3 will clear
    /// matching floor call buttons, let riders exit, and board waiting
    /// passengers here — in that order (research §3). Phase 2 arrivals
    /// carry no passengers, so staging is simply consumed.
    pub fn process_arrivals(&mut self) {
        self.pending_arrivals.clear();
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

    /// Presses an in-elevator destination button (Phase 3 passengers do
    /// this ~1 s after boarding), emitting
    /// [`Event::FloorButtonPressed`] on the unlit → lit transition.
    pub fn press_floor_button(&mut self, elevator: usize, floor: usize) {
        let elevator = self
            .elevators
            .get_mut(elevator)
            .expect("invariant: elevator index from this world");
        elevator.press_floor_button(floor, &mut self.events);
    }
}
