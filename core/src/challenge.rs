//! Challenge configurations, success conditions, and outcomes.
//!
//! A [`Challenge`] is the validated proof bundle a [`crate::World`] is
//! minted from (`proof-bundle` + `smart-constructor-newtype`): floor and
//! elevator counts, per-elevator capacities (cycling `capacities[i % len]`
//! like the original's `elevatorCapacities`), the spawn rate, and the
//! success [`Condition`]. The 19 playable configs and the three hidden
//! fitness configs are data here, verbatim from the original
//! `challenges.js` / `fitness.js`.
//!
//! Boundary semantics (research §4): a condition *triggers* at `>=` its
//! limits and *succeeds* at `<=` them, so hitting a limit exactly still
//! passes.

use crate::stats::Stats;

/// Why a challenge configuration was rejected.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Fewer than two floors: the spawn model needs somewhere to go.
    #[error("a challenge needs at least two floors, got {0}")]
    TooFewFloors(usize),
    /// No elevators at all.
    #[error("a challenge needs at least one elevator")]
    NoElevators,
    /// An empty capacities list has nothing to cycle.
    #[error("a challenge needs at least one elevator capacity")]
    NoCapacities,
    /// A zero-slot elevator could never board anyone.
    #[error("every elevator capacity needs at least one slot")]
    ZeroCapacity,
    /// Spawn rate must be finite and non-negative (zero disables
    /// spawning, for kinematics sandboxes).
    #[error("spawn rate must be finite and non-negative, got {0}")]
    InvalidSpawnRate(f64),
}

/// When a running challenge decides, and how (original `challenges.js`
/// condition templates). Evaluated after every substep's arrival
/// processing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Condition {
    /// "Transport N people in T seconds or less."
    UserCountWithinTime { user_count: usize, time_limit: f64 },
    /// "Transport N people and let no one wait more than W seconds" — no
    /// time limit; fails the instant anyone's wait reaches W.
    UserCountWithMaxWaitTime {
        user_count: usize,
        max_wait_time: f64,
    },
    /// Time limit *and* max-wait limit combined (challenge 18).
    Both {
        user_count: usize,
        time_limit: f64,
        max_wait_time: f64,
    },
    /// "Transport N people using M elevator moves or less."
    UserCountWithinMoves {
        user_count: usize,
        move_limit: usize,
    },
    /// Perpetual demo — never decides.
    Demo,
}

impl Condition {
    /// `None` = keep running; `Some(success)` the moment a trigger
    /// boundary is reached.
    pub fn evaluate(&self, stats: &Stats) -> Option<bool> {
        match *self {
            Condition::UserCountWithinTime {
                user_count,
                time_limit,
            } => (stats.elapsed() >= time_limit || stats.transported() >= user_count)
                .then(|| stats.transported() >= user_count),
            Condition::UserCountWithMaxWaitTime {
                user_count,
                max_wait_time,
            } => (stats.max_wait_time() >= max_wait_time || stats.transported() >= user_count)
                .then(|| {
                    stats.transported() >= user_count && stats.max_wait_time() <= max_wait_time
                }),
            Condition::Both {
                user_count,
                time_limit,
                max_wait_time,
            } => (stats.elapsed() >= time_limit
                || stats.max_wait_time() >= max_wait_time
                || stats.transported() >= user_count)
                .then(|| {
                    stats.transported() >= user_count && stats.max_wait_time() <= max_wait_time
                }),
            Condition::UserCountWithinMoves {
                user_count,
                move_limit,
            } => (stats.move_count() >= move_limit || stats.transported() >= user_count)
                .then(|| stats.transported() >= user_count && stats.move_count() <= move_limit),
            Condition::Demo => None,
        }
    }
}

/// How a challenge run stands. [`crate::World::ended`] is true once this
/// leaves [`Outcome::Running`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// No condition boundary reached yet.
    Running,
    /// The condition decided in the player's favor.
    Succeeded,
    /// The condition decided against the player.
    Failed,
}

/// A validated challenge configuration — the only thing a
/// [`crate::World`] can be minted from.
#[derive(Debug, Clone, PartialEq)]
pub struct Challenge {
    floor_count: usize,
    elevator_count: usize,
    capacities: Vec<usize>,
    spawn_rate: f64,
    condition: Condition,
}

impl Challenge {
    /// Validates a configuration into a challenge. A `spawn_rate` of zero
    /// is allowed and disables spawning entirely (useful for kinematics
    /// tests and sandboxes); the original never uses it.
    pub fn new(
        floor_count: usize,
        elevator_count: usize,
        capacities: Vec<usize>,
        spawn_rate: f64,
        condition: Condition,
    ) -> Result<Self, Error> {
        if floor_count < 2 {
            return Err(Error::TooFewFloors(floor_count));
        }
        if elevator_count == 0 {
            return Err(Error::NoElevators);
        }
        if capacities.is_empty() {
            return Err(Error::NoCapacities);
        }
        if capacities.contains(&0) {
            return Err(Error::ZeroCapacity);
        }
        if !spawn_rate.is_finite() || spawn_rate < 0.0 {
            return Err(Error::InvalidSpawnRate(spawn_rate));
        }
        Ok(Self {
            floor_count,
            elevator_count,
            capacities,
            spawn_rate,
            condition,
        })
    }

    /// Number of floors in the building.
    pub fn floor_count(&self) -> usize {
        self.floor_count
    }

    /// Number of elevators.
    pub fn elevator_count(&self) -> usize {
        self.elevator_count
    }

    /// The capacity cycle; see [`Challenge::capacity`].
    pub fn capacities(&self) -> &[usize] {
        &self.capacities
    }

    /// Capacity of elevator `elevator`: `capacities[elevator % len]`
    /// (challenge 10's elevators are 4, 10; challenge 18's are
    /// 6, 8, 6, 8, …).
    pub fn capacity(&self, elevator: usize) -> usize {
        self.capacities[elevator % self.capacities.len()]
    }

    /// Passengers spawned per game-second; zero disables spawning.
    pub fn spawn_rate(&self) -> f64 {
        self.spawn_rate
    }

    /// The success condition the minted world evaluates.
    pub fn condition(&self) -> Condition {
        self.condition
    }
}

/// The 19 playable challenges, verbatim from the original
/// `challenges.js`.
pub fn roster() -> Vec<Challenge> {
    let within_time = |user_count, time_limit| Condition::UserCountWithinTime {
        user_count,
        time_limit,
    };
    let max_wait = |user_count, max_wait_time| Condition::UserCountWithMaxWaitTime {
        user_count,
        max_wait_time,
    };
    let within_moves = |user_count, move_limit| Condition::UserCountWithinMoves {
        user_count,
        move_limit,
    };
    let challenge = |floors, elevators, capacities: &[usize], spawn_rate, condition| {
        Challenge::new(
            floors,
            elevators,
            capacities.to_vec(),
            spawn_rate,
            condition,
        )
        .expect("invariant: roster configs are valid")
    };
    vec![
        challenge(3, 1, &[4], 0.3, within_time(15, 60.0)),
        challenge(5, 1, &[4], 0.4, within_time(20, 60.0)),
        challenge(5, 1, &[6], 0.5, within_time(23, 60.0)),
        challenge(8, 2, &[4], 0.6, within_time(28, 60.0)),
        challenge(6, 4, &[4], 1.7, within_time(100, 68.0)),
        challenge(4, 2, &[4], 0.8, within_moves(40, 60)),
        challenge(3, 3, &[4], 3.0, within_moves(100, 63)),
        challenge(6, 2, &[5], 0.4, max_wait(50, 21.0)),
        challenge(7, 3, &[4], 0.6, max_wait(50, 20.0)),
        challenge(13, 2, &[4, 10], 1.1, within_time(50, 70.0)),
        challenge(9, 5, &[4], 1.1, max_wait(60, 19.0)),
        challenge(9, 5, &[4], 1.1, max_wait(80, 17.0)),
        challenge(9, 5, &[5], 1.1, max_wait(100, 15.0)),
        challenge(9, 5, &[6], 1.0, max_wait(110, 15.0)),
        challenge(8, 6, &[4], 0.9, max_wait(120, 14.0)),
        challenge(12, 4, &[5, 10], 1.4, within_time(70, 80.0)),
        challenge(21, 5, &[10], 1.9, within_time(110, 80.0)),
        challenge(
            21,
            8,
            &[6, 8],
            1.5,
            Condition::Both {
                user_count: 2675,
                time_limit: 1800.0,
                max_wait_time: 45.0,
            },
        ),
        challenge(21, 8, &[6, 8], 1.5, Condition::Demo),
    ]
}

/// The three hidden headless "fitness" scenarios (`fitness.js`),
/// small/medium/large. The original runs each for 12,000 fixed 1/60 s
/// frames to score average wait time; no success condition, no UI.
pub fn fitness() -> Vec<Challenge> {
    let scenario = |floors, elevators, capacity: usize, spawn_rate| {
        Challenge::new(
            floors,
            elevators,
            vec![capacity],
            spawn_rate,
            Condition::Demo,
        )
        .expect("invariant: fitness configs are valid")
    };
    vec![
        scenario(4, 2, 4, 0.6),
        scenario(6, 3, 5, 1.5),
        scenario(18, 6, 8, 1.9),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A `Stats` with the given fields, via the crate-internal mutators.
    fn stats(transported: usize, elapsed: f64, max_wait: f64, moves: usize) -> Stats {
        let mut stats = Stats::default();
        stats.advance(elapsed);
        for _ in 0..transported {
            stats.record_exit(0.0);
        }
        stats.observe_wait(max_wait);
        stats.set_move_count(moves);
        stats
    }

    #[test]
    fn time_condition_stays_undecided_below_both_boundaries() {
        let condition = Condition::UserCountWithinTime {
            user_count: 15,
            time_limit: 60.0,
        };
        assert_eq!(condition.evaluate(&stats(14, 59.9, 0.0, 0)), None);
    }

    #[test]
    fn hitting_a_limit_exactly_still_passes() {
        let condition = Condition::UserCountWithinTime {
            user_count: 15,
            time_limit: 60.0,
        };
        assert_eq!(condition.evaluate(&stats(15, 60.0, 0.0, 0)), Some(true));

        let condition = Condition::UserCountWithMaxWaitTime {
            user_count: 10,
            max_wait_time: 21.0,
        };
        assert_eq!(condition.evaluate(&stats(10, 30.0, 21.0, 0)), Some(true));

        let condition = Condition::UserCountWithinMoves {
            user_count: 40,
            move_limit: 60,
        };
        assert_eq!(condition.evaluate(&stats(40, 50.0, 0.0, 60)), Some(true));
    }

    #[test]
    fn crossing_a_wait_or_time_boundary_short_of_the_count_fails() {
        let condition = Condition::UserCountWithinTime {
            user_count: 15,
            time_limit: 60.0,
        };
        assert_eq!(condition.evaluate(&stats(14, 60.0, 0.0, 0)), Some(false));

        let condition = Condition::UserCountWithMaxWaitTime {
            user_count: 10,
            max_wait_time: 21.0,
        };
        assert_eq!(condition.evaluate(&stats(3, 30.0, 21.01, 0)), Some(false));
    }

    #[test]
    fn the_demo_condition_never_decides() {
        assert_eq!(
            Condition::Demo.evaluate(&stats(1000, 1e6, 1e5, 100000)),
            None
        );
    }

    #[test]
    fn capacities_cycle_across_elevators() {
        let challenge = &roster()[17];
        assert_eq!(challenge.elevator_count(), 8);
        let capacities: Vec<usize> = (0..8).map(|i| challenge.capacity(i)).collect();
        assert_eq!(capacities, vec![6, 8, 6, 8, 6, 8, 6, 8]);
    }

    #[test]
    fn invalid_configurations_are_rejected() {
        assert!(matches!(
            Challenge::new(1, 1, vec![4], 0.5, Condition::Demo),
            Err(Error::TooFewFloors(1))
        ));
        assert!(matches!(
            Challenge::new(3, 0, vec![4], 0.5, Condition::Demo),
            Err(Error::NoElevators)
        ));
        assert!(matches!(
            Challenge::new(3, 1, vec![], 0.5, Condition::Demo),
            Err(Error::NoCapacities)
        ));
        assert!(matches!(
            Challenge::new(3, 1, vec![4, 0], 0.5, Condition::Demo),
            Err(Error::ZeroCapacity)
        ));
        assert!(matches!(
            Challenge::new(3, 1, vec![4], -1.0, Condition::Demo),
            Err(Error::InvalidSpawnRate(_))
        ));
    }
}
