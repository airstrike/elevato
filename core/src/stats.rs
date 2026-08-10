//! Live simulation statistics, exactly as the original computes them.
//!
//! Wait time is *time since spawn* - it includes riding and the post-exit
//! walk-off until removal. `max_wait_time` therefore climbs every step
//! while anyone is present; `avg_wait_time` is an incremental mean taken
//! at each exit; `transported_per_sec` updates only at exits (it does not
//! decay between them). Challenge conditions read this snapshot after
//! every substep's arrival processing.

/// A snapshot of the world's running statistics. Plain data - comparing
/// two runs' final stats with `==` is the determinism check.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Stats {
    transported: usize,
    elapsed: f64,
    transported_per_sec: f64,
    max_wait_time: f64,
    avg_wait_time: f64,
    move_count: usize,
}

impl Stats {
    /// Passengers who have exited at their destination.
    pub fn transported(&self) -> usize {
        self.transported
    }

    /// Simulated seconds advanced so far.
    pub fn elapsed(&self) -> f64 {
        self.elapsed
    }

    /// `transported / elapsed`, refreshed at each exit.
    pub fn transported_per_sec(&self) -> f64 {
        self.transported_per_sec
    }

    /// Longest wait observed so far, over exits *and* everyone still
    /// present (refreshed every step).
    pub fn max_wait_time(&self) -> f64 {
        self.max_wait_time
    }

    /// Incremental mean of wait time at the moment of each exit.
    pub fn avg_wait_time(&self) -> f64 {
        self.avg_wait_time
    }

    /// Floor boundaries crossed, summed over all elevators.
    pub fn move_count(&self) -> usize {
        self.move_count
    }

    /// Advances simulated time by one substep.
    pub(crate) fn advance(&mut self, dt: f64) {
        self.elapsed += dt;
    }

    /// Folds one present passenger's current wait into the maximum - the
    /// every-step refresh that keeps `max_wait_time` climbing while
    /// anyone waits, rides, or walks off.
    pub(crate) fn observe_wait(&mut self, wait: f64) {
        self.max_wait_time = self.max_wait_time.max(wait);
    }

    /// Records one passenger exiting with the given wait: transported
    /// count, per-second rate, maximum, and the incremental average
    /// `(avg·(n−1) + wait) / n`.
    pub(crate) fn record_exit(&mut self, wait: f64) {
        self.transported += 1;
        self.transported_per_sec = self.transported as f64 / self.elapsed;
        self.max_wait_time = self.max_wait_time.max(wait);
        let n = self.transported as f64;
        self.avg_wait_time = (self.avg_wait_time * (n - 1.0) + wait) / n;
    }

    /// Refreshes the elevator move-count sum after a physics step.
    pub(crate) fn set_move_count(&mut self, move_count: usize) {
        self.move_count = move_count;
    }
}
