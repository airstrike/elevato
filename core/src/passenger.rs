//! Passengers: the spawn-to-removal lifecycle with the original's
//! load-bearing walk timers.
//!
//! A passenger spawns waiting on a floor, boards by walking to an elevator
//! slot over exactly 1.0 s (only then pressing the in-elevator destination
//! button), rides, and on arriving at its destination exits and walks off
//! over `1.0 + rand·0.5` s before removal. Wait time — `elapsed − spawn
//! time` — keeps feeding the every-step `max_wait_time` refresh through
//! the entire lifecycle, walk-off included; mid-walk ("busy") passengers
//! are skipped by boarding attempts.

/// Seconds a boarder spends walking to its slot before pressing the
/// in-elevator destination button.
const BOARD_WALK: f64 = 1.0;

/// Cosmetic passenger appearance, drawn at spawn. The draw itself is
/// load-bearing — it keeps the RNG stream aligned — even though nothing in
/// the simulation reads the result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    /// 1-in-41 chance per spawn.
    Child,
    /// Half of the remaining spawns.
    Female,
    /// The other half.
    Male,
}

/// Where a passenger is in its lifecycle. Timers count down in
/// [`Passenger::tick`]; the world reacts to completions.
#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    /// On a floor, eligible to board a suitable arrival.
    Waiting,
    /// Aboard, walking to its slot — busy, skipped by boarding and not yet
    /// pressing the destination button.
    Boarding {
        elevator: usize,
        slot: usize,
        walk_remaining: f64,
    },
    /// Aboard with the destination button pressed, riding until the
    /// elevator stops at the destination floor.
    Riding { elevator: usize, slot: usize },
    /// Exited at the destination (already counted as transported), walking
    /// off until removal.
    Exiting { walk_remaining: f64 },
}

/// A timer that ran out during [`Passenger::tick`], for the world to act
/// on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Completion {
    /// The walk to the slot finished: the passenger now presses the
    /// elevator's destination button.
    BoardWalk { elevator: usize, destination: usize },
    /// The exit walk finished: remove the passenger from the world.
    ExitWalk,
}

/// One passenger, minted by the world's seeded spawn stream.
#[derive(Debug, Clone, PartialEq)]
pub struct Passenger {
    weight: u32,
    appearance: Appearance,
    current_floor: usize,
    destination_floor: usize,
    spawn_time: f64,
    state: State,
}

impl Passenger {
    pub(crate) fn new(
        weight: u32,
        appearance: Appearance,
        spawn_floor: usize,
        destination_floor: usize,
        spawn_time: f64,
    ) -> Self {
        Self {
            weight,
            appearance,
            current_floor: spawn_floor,
            destination_floor,
            spawn_time,
            state: State::Waiting,
        }
    }

    /// Body weight, a uniform integer in 55–100. Feeds
    /// [`crate::elevator::Elevator::load_factor`] only — never blocks
    /// boarding.
    pub fn weight(&self) -> u32 {
        self.weight
    }

    /// Cosmetic appearance drawn at spawn.
    pub fn appearance(&self) -> Appearance {
        self.appearance
    }

    /// The floor the passenger is on (or boarded from, while aboard).
    /// Updated to the destination on exit.
    pub fn current_floor(&self) -> usize {
        self.current_floor
    }

    /// Where the passenger wants to go. Never equals the spawn floor.
    pub fn destination_floor(&self) -> usize {
        self.destination_floor
    }

    /// Simulated time at spawn; wait time is `elapsed − spawn_time`.
    pub fn spawn_time(&self) -> f64 {
        self.spawn_time
    }

    /// Whether the passenger is waiting on a floor, eligible to board.
    pub fn is_waiting(&self) -> bool {
        matches!(self.state, State::Waiting)
    }

    /// The elevator the passenger is aboard (walking to its slot or
    /// riding), if any.
    pub fn aboard(&self) -> Option<usize> {
        match self.state {
            State::Boarding { elevator, .. } | State::Riding { elevator, .. } => Some(elevator),
            State::Waiting | State::Exiting { .. } => None,
        }
    }

    /// Whether the passenger has exited (already transported) and is
    /// walking off before removal.
    pub fn is_walking_off(&self) -> bool {
        matches!(self.state, State::Exiting { .. })
    }

    /// The elevator slot occupied while aboard (boarding or riding), if
    /// any — read by the world's exit path and by renderers placing
    /// riders in their slots.
    pub fn slot(&self) -> Option<usize> {
        match self.state {
            State::Boarding { slot, .. } | State::Riding { slot, .. } => Some(slot),
            State::Waiting | State::Exiting { .. } => None,
        }
    }

    /// Boards: the slot is already occupied on the elevator's side; the
    /// 1.0 s walk to it starts now.
    pub(crate) fn board(&mut self, elevator: usize, slot: usize) {
        self.state = State::Boarding {
            elevator,
            slot,
            walk_remaining: BOARD_WALK,
        };
    }

    /// Exits at `floor` (the destination), starting a `walk` -second
    /// walk-off. The elevator slot is already freed on the elevator's
    /// side; stats record the exit at this moment.
    pub(crate) fn exit(&mut self, floor: usize, walk: f64) {
        self.current_floor = floor;
        self.state = State::Exiting {
            walk_remaining: walk,
        };
    }

    /// Advances walk timers by one substep, reporting a timer that ran
    /// out. Completion is strictly-past, mirroring the dwell timer: a walk
    /// completes on the first substep that pushes it *below* zero.
    pub(crate) fn tick(&mut self, dt: f64) -> Option<Completion> {
        match &mut self.state {
            State::Waiting | State::Riding { .. } => None,
            State::Boarding {
                elevator,
                slot,
                walk_remaining,
            } => {
                *walk_remaining -= dt;
                if *walk_remaining < 0.0 {
                    let (elevator, slot) = (*elevator, *slot);
                    self.state = State::Riding { elevator, slot };
                    Some(Completion::BoardWalk {
                        elevator,
                        destination: self.destination_floor,
                    })
                } else {
                    None
                }
            }
            State::Exiting { walk_remaining } => {
                *walk_remaining -= dt;
                (*walk_remaining < 0.0).then_some(Completion::ExitWalk)
            }
        }
    }
}
