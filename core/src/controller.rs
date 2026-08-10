//! The boundary anything driving a world implements - Rhai in Phase 4,
//! native Rust strategies in tests and benchmarks.
//!
//! A controller receives `&mut World` everywhere so it can issue commands
//! ([`crate::World::go_to_floor`] and friends) directly; the driver
//! ([`crate::headless::run`]) owns the substep/dispatch loop and calls the
//! three hooks at the moments the original called user code.

use crate::World;
use crate::event::Event;

/// User-strategy hooks, mirroring the original's `{ init, update }` code
/// object plus its event subscriptions.
pub trait Controller {
    /// Called once before the first frame. Right after it returns, the
    /// driver checks every elevator's destination queue, firing the
    /// initial [`Event::Idle`] round exactly like the original's
    /// `world.init()`.
    fn init(&mut self, world: &mut World);

    /// Called once per frame with the *whole* frame dt, before that
    /// frame's physics substeps - commands issued here take effect in this
    /// frame. Optional; the default does nothing.
    fn update(&mut self, world: &mut World, dt: f64) {
        let _ = (world, dt);
    }

    /// Called for every event drained between substeps, in emission
    /// order. Optional; the default does nothing.
    fn on_event(&mut self, world: &mut World, event: Event) {
        let _ = (world, event);
    }
}
