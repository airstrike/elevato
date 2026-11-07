//! The playback state machine: owns the [`Simulation`] running the
//! player's program, the run/pause flag, the integer timescale, and the
//! seed policy. The app screen is a thin TEA shell over this type, so
//! live playback can be tested headlessly (`tests/playback.rs` is the
//! fixed-timestep determinism gate).
//!
//! # Seed policy
//!
//! The original game is unseeded - every run is different and none is
//! reproducible. Elevato instead seeds each run from an **attempt
//! counter** starting at 1 and incremented on every restart and
//! challenge switch: consecutive attempts vary like the original's, yet
//! any single run is reproducible from (challenge, seed, program). The
//! app displays the seed in its stats bar so a run can be referenced.
//!
//! An edit that reaches the program mid-run breaks that: the run is
//! then [`Playback::edited`], and the app marks its seed so. A restart
//! is the clean path back, booting the program as it now stands.
//!
//! # Timescale
//!
//! An integer in `[1, 39]`, default 2, stepped by the original's
//! rounded golden-ratio ladder: `round(timescale × 1.618)` up,
//! `round(timescale / 1.618)` down, clamped ("capped below 40"). One
//! [`Playback::tick`] advances the simulation by exactly `timescale`
//! substeps of [`DT_MAX`](crate::core::world::DT_MAX) - the
//! fixed-timestep deviation that makes a live run replay a headless run
//! byte for byte.

use std::sync::Arc;

use elevato_api::simulation::{self, Simulation};
use elevato_api::{Elevator, Floor};
use iced::{Subscription, Task};

use crate::core::World;
use crate::core::challenge::{self, Challenge, Outcome};
use crate::core::stats::Stats;

/// The built-in starter program - the Rust port of the original's
/// default implementation, which Reset restores. The canonical copy
/// lives with the solutions and is included verbatim.
pub const STARTER: &str = include_str!("../tests/solutions/starter.rs");

/// The original's timescale stepping ratio.
const TIMESCALE_RATIO: f64 = 1.618;

/// Slowest playback: one substep per tick.
const TIMESCALE_MIN: usize = 1;

/// Fastest playback (the original caps "below 40").
const TIMESCALE_MAX: usize = 39;

/// The player's program, ready to boot on any challenge: its `new`,
/// `update` and `subscription`, whatever its state and message types.
#[derive(Clone)]
pub struct Program {
    boot: Arc<Boot>,
}

type Boot = dyn Fn(&Challenge, u64) -> Result<Box<dyn Run>, simulation::Error>;

/// A [`Simulation`] of the player's program, seen without its state and
/// message types.
pub trait Run {
    /// Advances one frame of `substeps` substeps.
    fn frame(&mut self, substeps: usize) -> Result<(), simulation::Error>;
    /// The live statistics.
    fn stats(&self) -> Stats;
    /// How the challenge stands.
    fn outcome(&self) -> Outcome;
    /// Whether the challenge has decided.
    fn ended(&self) -> bool;
    /// The world, for drawing.
    fn world(&self) -> &World;
}

impl<P, M> Run for Simulation<P, M>
where
    M: Send + 'static,
{
    fn frame(&mut self, substeps: usize) -> Result<(), simulation::Error> {
        Simulation::frame(self, substeps)
    }

    fn stats(&self) -> Stats {
        Simulation::stats(self)
    }

    fn outcome(&self) -> Outcome {
        Simulation::outcome(self)
    }

    fn ended(&self) -> bool {
        Simulation::ended(self)
    }

    fn world(&self) -> &World {
        Simulation::world(self)
    }
}

impl Program {
    /// A program from its three functions, as iced takes an
    /// application's.
    pub fn new<P, M>(
        new: impl Fn(Vec<Elevator>, Vec<Floor>) -> (P, Task<M>) + 'static,
        update: impl Fn(&mut P, M) -> Task<M> + Clone + 'static,
        subscription: impl Fn(&P) -> Subscription<M> + Clone + 'static,
    ) -> Self
    where
        P: 'static,
        M: Send + 'static,
    {
        Self {
            boot: Arc::new(move |challenge: &Challenge, seed| {
                let simulation =
                    Simulation::new(challenge, seed, &new, update.clone(), subscription.clone())?;
                Ok(Box::new(simulation) as Box<dyn Run>)
            }),
        }
    }

    /// A fresh run on `challenge`, seeded with `seed`.
    pub fn boot(
        &self,
        challenge: &Challenge,
        seed: u64,
    ) -> Result<Box<dyn Run>, simulation::Error> {
        (self.boot)(challenge, seed)
    }
}

/// A playable simulation: the challenge roster and the live [`Run`] of
/// the player's program for the current attempt. Constructed paused on
/// challenge 1; [`Playback::tick`] advances it while running.
pub struct Playback {
    program: Program,
    roster: Vec<Challenge>,
    challenge: usize,
    seed: u64,
    timescale: usize,
    running: bool,
    run: Option<Box<dyn Run>>,
    error: Option<simulation::Error>,
    /// Whether code changed under the running attempt.
    edited: bool,
}

impl Playback {
    /// Builds the first attempt of `program` (challenge 1, seed 1,
    /// timescale 2),
    /// paused until [`Playback::toggle`].
    pub fn new(program: Program) -> Self {
        let mut playback = Self {
            program,
            roster: challenge::roster(),
            challenge: 0,
            seed: 1,
            timescale: 2,
            running: false,
            run: None,
            error: None,
            edited: false,
        };
        playback.rebuild();
        playback
    }

    /// The challenge roster the picker offers, in play order.
    pub fn challenges(&self) -> &[Challenge] {
        &self.roster
    }

    /// Index of the current challenge in [`Playback::challenges`].
    pub fn challenge_index(&self) -> usize {
        self.challenge
    }

    /// The current attempt's seed (see the module docs for the policy).
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Substeps advanced per tick.
    pub fn timescale(&self) -> usize {
        self.timescale
    }

    /// Whether ticks currently advance the simulation.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// The live statistics snapshot (zeroed when no run exists).
    pub fn stats(&self) -> Stats {
        self.run.as_ref().map(|run| run.stats()).unwrap_or_default()
    }

    /// How the current attempt stands.
    pub fn outcome(&self) -> Outcome {
        self.run
            .as_ref()
            .map(|run| run.outcome())
            .unwrap_or(Outcome::Running)
    }

    /// Whether the current attempt has decided.
    pub fn ended(&self) -> bool {
        self.run.as_ref().is_some_and(|run| run.ended())
    }

    /// The program's panic that stopped the attempt, if one did.
    pub fn error(&self) -> Option<&simulation::Error> {
        self.error.as_ref()
    }

    /// Whether the program's code changed while this attempt ran, so
    /// the seed and the program on disk no longer reproduce it.
    pub fn edited(&self) -> bool {
        self.edited
    }

    /// Records that new code took over the running attempt.
    pub fn edit(&mut self) {
        if self.run.is_some() {
            self.edited = true;
        }
    }

    /// Read access to the world, for rendering.
    pub fn world(&self) -> Option<&World> {
        self.run.as_deref().map(Run::world)
    }

    /// Starts or pauses playback. Ignored once the attempt has decided
    /// (restart or pick the next challenge instead) or after the
    /// program panicked.
    pub fn toggle(&mut self) {
        if self.ended() || self.run.is_none() || self.error.is_some() {
            return;
        }
        self.running = !self.running;
    }

    /// Tears the attempt down and starts a fresh one on the same
    /// challenge with the next seed, auto-started like the original's
    /// restart flow. The program boots as it now stands, so the fresh
    /// attempt is reproducible again.
    pub fn restart(&mut self) {
        self.seed += 1;
        self.rebuild();
        self.running = self.run.is_some();
    }

    /// Switches to the challenge at `index`, advancing the seed and
    /// leaving the fresh attempt paused. Out-of-roster indices are
    /// ignored.
    pub fn select_challenge(&mut self, index: usize) {
        if index >= self.roster.len() {
            return;
        }
        self.challenge = index;
        self.seed += 1;
        self.rebuild();
        self.running = false;
    }

    /// Advances to the next challenge (the success banner's button and
    /// the Cmd+Page Down hotkey); a no-op on the last one.
    pub fn next_challenge(&mut self) {
        if self.challenge + 1 < self.roster.len() {
            self.select_challenge(self.challenge + 1);
        }
    }

    /// Steps back to the previous challenge (Cmd+Page Up); a no-op on
    /// the first one.
    pub fn previous_challenge(&mut self) {
        if let Some(previous) = self.challenge.checked_sub(1) {
            self.select_challenge(previous);
        }
    }

    /// Restores a persisted timescale, clamped to the ladder's bounds.
    pub fn set_timescale(&mut self, timescale: usize) {
        self.timescale = timescale.clamp(TIMESCALE_MIN, TIMESCALE_MAX);
    }

    /// Steps the timescale up the golden-ratio ladder.
    pub fn speed_up(&mut self) {
        self.timescale = step(self.timescale, TIMESCALE_RATIO);
    }

    /// Steps the timescale down the golden-ratio ladder.
    pub fn slow_down(&mut self) {
        self.timescale = step(self.timescale, 1.0 / TIMESCALE_RATIO);
    }

    /// Advances one frame of `timescale` substeps, when running. A
    /// panic in the program pauses playback and is surfaced via
    /// [`Playback::error`]; a decided challenge pauses playback so the
    /// driving tick subscription can stop.
    pub fn tick(&mut self) {
        if !self.running {
            return;
        }
        let Some(run) = &mut self.run else {
            self.running = false;
            return;
        };
        if let Err(error) = run.frame(self.timescale) {
            self.error = Some(error);
            self.running = false;
            return;
        }
        if run.ended() {
            self.running = false;
        }
    }

    /// Builds the run for the current (challenge, seed) - dropping the
    /// previous attempt first, so its world is freed before the new one
    /// exists. A panic in the program's `new` leaves no run and records
    /// the error.
    fn rebuild(&mut self) {
        self.error = None;
        self.edited = false;
        self.run = None;
        match self.program.boot(&self.roster[self.challenge], self.seed) {
            Ok(run) => self.run = Some(run),
            Err(error) => {
                self.error = Some(error);
                self.running = false;
            }
        }
    }
}

/// One rounded, clamped step of the timescale ladder.
fn step(timescale: usize, factor: f64) -> usize {
    let stepped = (timescale as f64 * factor).round() as usize;
    stepped.clamp(TIMESCALE_MIN, TIMESCALE_MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use elevato_api::elevator;

    /// A program that shuttles every car between the lobby and the top.
    struct Shuttle {
        floors: Vec<Floor>,
    }

    fn shuttle() -> Program {
        Program::new(
            |elevators: Vec<Elevator>, floors: Vec<Floor>| {
                let top = floors[floors.len() - 1];
                let task = Task::batch(elevators.into_iter().map(|elevator| {
                    elevator
                        .go_to_floor(top)
                        .map(move |message| (elevator, message))
                }));
                (Shuttle { floors }, task)
            },
            |shuttle: &mut Shuttle, (elevator, message): (Elevator, elevator::Message)| {
                match message {
                    elevator::Message::StoppedAtFloor { floor, .. } => {
                        let next = if floor == shuttle.floors[0] {
                            shuttle.floors[shuttle.floors.len() - 1]
                        } else {
                            shuttle.floors[0]
                        };
                        elevator
                            .go_to_floor(next)
                            .map(move |message| (elevator, message))
                    }
                    elevator::Message::FloorButtonPressed(_)
                    | elevator::Message::PassingFloor(..) => Task::none(),
                }
            },
            |_: &Shuttle| Subscription::none(),
        )
    }

    #[test]
    fn the_timescale_climbs_the_golden_ratio_ladder_and_clamps_at_thirty_nine() {
        let mut playback = Playback::new(shuttle());
        assert_eq!(playback.timescale(), 2);
        let ladder: Vec<usize> = (0..8)
            .map(|_| {
                playback.speed_up();
                playback.timescale()
            })
            .collect();
        assert_eq!(ladder, vec![3, 5, 8, 13, 21, 34, 39, 39]);
    }

    #[test]
    fn the_timescale_descends_the_ladder_and_clamps_at_one() {
        let mut playback = Playback::new(shuttle());
        for _ in 0..8 {
            playback.speed_up();
        }
        assert_eq!(playback.timescale(), 39);
        let ladder: Vec<usize> = (0..8)
            .map(|_| {
                playback.slow_down();
                playback.timescale()
            })
            .collect();
        assert_eq!(ladder, vec![24, 15, 9, 6, 4, 2, 1, 1]);
    }

    #[test]
    fn the_timescale_never_leaves_its_bounds() {
        let mut playback = Playback::new(shuttle());
        for _ in 0..20 {
            playback.speed_up();
            assert!((TIMESCALE_MIN..=TIMESCALE_MAX).contains(&playback.timescale()));
        }
        for _ in 0..20 {
            playback.slow_down();
            assert!((TIMESCALE_MIN..=TIMESCALE_MAX).contains(&playback.timescale()));
        }
        assert_eq!(playback.timescale(), TIMESCALE_MIN);
    }

    #[test]
    fn every_restart_and_challenge_switch_advances_the_seed() {
        let mut playback = Playback::new(shuttle());
        assert_eq!(playback.seed(), 1);
        playback.restart();
        assert_eq!(playback.seed(), 2);
        playback.select_challenge(3);
        assert_eq!(playback.seed(), 3);
        assert_eq!(playback.challenge_index(), 3);
    }

    #[test]
    fn a_restart_rebuilds_the_same_challenge_with_the_next_seed_and_autostarts() {
        let mut playback = Playback::new(shuttle());
        playback.select_challenge(2);
        playback.toggle();
        playback.tick();
        assert!(playback.stats().elapsed() > 0.0);

        playback.restart();
        assert_eq!(playback.challenge_index(), 2);
        assert_eq!(playback.seed(), 3);
        assert!(playback.is_running(), "a restart auto-starts the attempt");
        assert_eq!(playback.stats().elapsed(), 0.0, "the world is fresh");
        assert!(playback.error().is_none());
    }

    #[test]
    fn an_edit_marks_the_attempt_until_a_restart() {
        let mut playback = Playback::new(shuttle());
        assert!(!playback.edited());
        playback.edit();
        assert!(playback.edited());
        playback.tick();
        assert!(playback.edited(), "the mark lasts the attempt");
        playback.restart();
        assert!(!playback.edited(), "a restart is reproducible again");
        playback.edit();
        playback.select_challenge(4);
        assert!(!playback.edited());
    }

    #[test]
    fn a_restored_timescale_is_clamped_to_the_ladder_bounds() {
        let mut playback = Playback::new(shuttle());
        playback.set_timescale(500);
        assert_eq!(playback.timescale(), TIMESCALE_MAX);
        playback.set_timescale(0);
        assert_eq!(playback.timescale(), TIMESCALE_MIN);
        playback.set_timescale(7);
        assert_eq!(playback.timescale(), 7);
    }

    #[test]
    fn a_fresh_playback_is_paused_and_ticks_do_nothing_until_started() {
        let mut playback = Playback::new(shuttle());
        assert!(!playback.is_running());
        playback.tick();
        assert_eq!(playback.stats().elapsed(), 0.0);
        playback.toggle();
        assert!(playback.is_running());
        playback.tick();
        assert!(playback.stats().elapsed() > 0.0);
    }
}
