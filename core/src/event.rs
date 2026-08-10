//! Typed simulation events.
//!
//! The core is callback-free: stepping and commands append events to the
//! world's drainable queue, and the driver dispatches them between substeps.
//! Elevators are identified by their index in the world's elevator list.

/// Vertical travel direction in floor space: up means toward higher levels,
/// which in the y-down pixel space means *decreasing* y (negative velocity).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Toward higher floors (negative y velocity).
    Up,
    /// Toward lower floors (positive y velocity).
    Down,
}

/// A simulation occurrence, drained by the driver via
/// [`crate::World::drain_events`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// The elevator's destination queue was checked while empty and not
    /// busy. Fired by an explicit queue check and ~1 s after the last
    /// arrival's dwell completes with nothing queued.
    Idle { elevator: usize },
    /// The elevator is about to pass a floor it is not stopping at.
    /// Detected from the braking-distance projection: fired when
    /// `trunc(future_floor_if_stopped)` changes, never for the current
    /// destination floor.
    PassingFloor {
        elevator: usize,
        floor: usize,
        direction: Direction,
    },
    /// The elevator physically arrived and snapped exactly onto a floor.
    /// Fired before exit/boarding processing: the driver dispatches this,
    /// then calls [`crate::World::process_arrivals`].
    StoppedAtFloor { elevator: usize, floor: usize },
    /// A destination button inside the elevator went from unlit to lit.
    /// Pressing an already-lit button emits nothing.
    FloorButtonPressed { elevator: usize, floor: usize },
    /// A floor's up call button went from unlit to lit - a passenger
    /// spawned wanting to go up, or an overflow passenger re-pressed after
    /// an arrival cleared the state. Pressing a lit button emits nothing.
    UpButtonPressed { floor: usize },
    /// A floor's down call button went from unlit to lit; the down twin of
    /// [`Event::UpButtonPressed`].
    DownButtonPressed { floor: usize },
}
