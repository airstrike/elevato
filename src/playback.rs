//! The iced-free playback state machine: owns the script runtime, the
//! run/pause flag, the integer timescale, and the seed policy. The app
//! screen is a thin TEA shell over this type, so live playback can be
//! tested headlessly (`tests/playback.rs` is the fixed-timestep
//! determinism gate).
//!
//! # Seed policy
//!
//! The original game is unseeded — every run is different and none is
//! reproducible. Elevato instead seeds each run from an **attempt
//! counter** starting at 1 and incremented on every restart and
//! challenge switch: consecutive attempts vary like the original's, yet
//! any single run is reproducible from (challenge, seed, script). The
//! app displays the seed in its stats bar so a run can be referenced.
//!
//! # Timescale
//!
//! An integer in `[1, 39]`, default 2, stepped by the original's
//! rounded golden-ratio ladder: `round(timescale × 1.618)` up,
//! `round(timescale / 1.618)` down, clamped ("capped below 40"). One
//! [`Playback::tick`] advances the runtime by exactly `timescale`
//! substeps of [`DT_MAX`](elevato_core::world::DT_MAX) — the
//! fixed-timestep deviation that makes a live run replay a headless run
//! byte for byte.

use std::cell::Ref;

use crate::core::World;
use crate::core::challenge::{self, Challenge, Outcome};
use crate::core::stats::Stats;

use script::{Program, Runtime};

/// The built-in starter program — the rhai port of the original's
/// default implementation. The canonical copy lives with the script
/// crate's solution fixtures and is included verbatim.
pub const STARTER: &str = include_str!("../script/tests/solutions/starter.rhai");

/// The original's timescale stepping ratio.
const TIMESCALE_RATIO: f64 = 1.618;

/// Slowest playback: one substep per tick.
const TIMESCALE_MIN: usize = 1;

/// Fastest playback (the original caps "below 40").
const TIMESCALE_MAX: usize = 39;

/// A playable simulation: a compiled program, the challenge roster, and
/// the live [`Runtime`] for the current attempt. Constructed paused on
/// challenge 1; [`Playback::tick`] advances it while running.
pub struct Playback {
    program: Program,
    roster: Vec<Challenge>,
    challenge: usize,
    seed: u64,
    timescale: usize,
    running: bool,
    runtime: Option<Runtime>,
    error: Option<script::Error>,
}

impl Playback {
    /// Compiles `source` and builds the first attempt (challenge 1,
    /// seed 1, timescale 2), paused until [`Playback::toggle`].
    pub fn new(source: &str) -> Result<Self, script::Error> {
        let mut playback = Self {
            program: Program::compile(source)?,
            roster: challenge::roster(),
            challenge: 0,
            seed: 1,
            timescale: 2,
            running: false,
            runtime: None,
            error: None,
        };
        playback.rebuild();
        Ok(playback)
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

    /// The live statistics snapshot (zeroed when no runtime exists).
    pub fn stats(&self) -> Stats {
        self.runtime
            .as_ref()
            .map(Runtime::stats)
            .unwrap_or_default()
    }

    /// How the current attempt stands.
    pub fn outcome(&self) -> Outcome {
        self.runtime
            .as_ref()
            .map(Runtime::outcome)
            .unwrap_or(Outcome::Running)
    }

    /// Whether the current attempt has decided.
    pub fn ended(&self) -> bool {
        self.runtime.as_ref().is_some_and(Runtime::ended)
    }

    /// The script error that stopped the attempt, if one did.
    pub fn error(&self) -> Option<&script::Error> {
        self.error.as_ref()
    }

    /// Read access to the world, for rendering. The borrow must not be
    /// held across any `&mut self` call.
    pub fn world(&self) -> Option<Ref<'_, World>> {
        self.runtime.as_ref().map(Runtime::world)
    }

    /// Starts or pauses playback. Ignored once the attempt has decided
    /// (restart or pick the next challenge instead) or after a script
    /// error.
    pub fn toggle(&mut self) {
        if self.ended() || self.runtime.is_none() {
            return;
        }
        self.running = !self.running;
    }

    /// Tears the attempt down and starts a fresh one on the same
    /// challenge with the next seed, auto-started like the original's
    /// restart flow.
    pub fn restart(&mut self) {
        self.seed += 1;
        self.rebuild();
        self.running = self.runtime.is_some();
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

    /// Compiles `source` and, on success, replaces the program and
    /// starts a fresh attempt on the **current** challenge with the
    /// next seed, auto-started (the original's Apply flow — nothing
    /// survives but editor text and timescale). On a compile error the
    /// running attempt is left untouched but paused, and the error is
    /// returned for the caller to surface.
    pub fn apply(&mut self, source: &str) -> Result<(), script::Error> {
        match Program::compile(source) {
            Ok(program) => {
                self.program = program;
                self.seed += 1;
                self.rebuild();
                self.running = self.runtime.is_some();
                Ok(())
            }
            Err(error) => {
                self.running = false;
                Err(error)
            }
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
    /// script error pauses playback and is surfaced via
    /// [`Playback::error`]; a decided challenge pauses playback so the
    /// driving tick subscription can stop.
    pub fn tick(&mut self) {
        if !self.running {
            return;
        }
        let Some(runtime) = &mut self.runtime else {
            self.running = false;
            return;
        };
        if let Err(error) = runtime.frame(self.timescale) {
            self.error = Some(error);
            self.running = false;
            return;
        }
        if runtime.ended() {
            self.running = false;
        }
    }

    /// Builds the runtime for the current (challenge, seed) — dropping
    /// the previous attempt first, so its world is freed before the new
    /// one exists. A construction failure (`init` threw) leaves no
    /// runtime and records the error.
    fn rebuild(&mut self) {
        self.error = None;
        self.runtime = None;
        match Runtime::new(
            self.program.clone(),
            &self.roster[self.challenge],
            self.seed,
        ) {
            Ok(runtime) => self.runtime = Some(runtime),
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

    #[test]
    fn the_timescale_climbs_the_golden_ratio_ladder_and_clamps_at_thirty_nine() {
        let mut playback = Playback::new(STARTER).unwrap();
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
        let mut playback = Playback::new(STARTER).unwrap();
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
        let mut playback = Playback::new(STARTER).unwrap();
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
        let mut playback = Playback::new(STARTER).unwrap();
        assert_eq!(playback.seed(), 1);
        playback.restart();
        assert_eq!(playback.seed(), 2);
        playback.select_challenge(3);
        assert_eq!(playback.seed(), 3);
        assert_eq!(playback.challenge_index(), 3);
    }

    #[test]
    fn a_successful_apply_rebuilds_the_same_challenge_with_the_next_seed_and_autostarts() {
        let mut playback = Playback::new(STARTER).unwrap();
        playback.select_challenge(2);
        playback.toggle();
        playback.tick();
        assert!(playback.stats().elapsed() > 0.0);

        playback.apply("fn init(elevators, floors) {}").unwrap();
        assert_eq!(playback.challenge_index(), 2);
        assert_eq!(playback.seed(), 3);
        assert!(playback.is_running(), "apply auto-starts the new attempt");
        assert_eq!(playback.stats().elapsed(), 0.0, "the world is fresh");
        assert!(playback.error().is_none());
    }

    #[test]
    fn a_compile_error_apply_pauses_but_preserves_the_current_attempt() {
        let mut playback = Playback::new(STARTER).unwrap();
        playback.toggle();
        playback.tick();
        let elapsed = playback.stats().elapsed();
        assert!(elapsed > 0.0);

        let error = playback.apply("fn init(").unwrap_err();
        assert!(matches!(error, script::Error::Compile(_)));
        assert!(!playback.is_running(), "a failed apply pauses playback");
        assert!(
            playback.error().is_none(),
            "the compile error belongs to the caller, not the attempt"
        );
        assert_eq!(playback.seed(), 1, "the attempt was not rebuilt");
        assert_eq!(playback.stats().elapsed(), elapsed);

        // The old attempt is still alive: resume and it keeps running.
        playback.toggle();
        playback.tick();
        assert!(playback.stats().elapsed() > elapsed);
    }

    #[test]
    fn a_restored_timescale_is_clamped_to_the_ladder_bounds() {
        let mut playback = Playback::new(STARTER).unwrap();
        playback.set_timescale(500);
        assert_eq!(playback.timescale(), TIMESCALE_MAX);
        playback.set_timescale(0);
        assert_eq!(playback.timescale(), TIMESCALE_MIN);
        playback.set_timescale(7);
        assert_eq!(playback.timescale(), 7);
    }

    #[test]
    fn a_fresh_playback_is_paused_and_ticks_do_nothing_until_started() {
        let mut playback = Playback::new(STARTER).unwrap();
        assert!(!playback.is_running());
        playback.tick();
        assert_eq!(playback.stats().elapsed(), 0.0);
        playback.toggle();
        assert!(playback.is_running());
        playback.tick();
        assert!(playback.stats().elapsed() > 0.0);
    }
}
