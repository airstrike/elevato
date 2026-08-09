//! ─────────────────────────────────────────────────────────────────
//! elevato.rs — the scripting API
//!
//! This reference is code: cmd+click any name — here or in your own
//! program — to jump to its definition. `Elevator` and `Floor` open
//! their own pages; cmd+click keeps working there.
//! ─────────────────────────────────────────────────────────────────

/// Called once, when the challenge starts.
///
/// Required — a program without `init` does not compile. This is
/// where handlers are bound and shared state is declared.
fn init(elevators: Vec<Elevator>, floors: Vec<Floor>);

/// Called once per frame, before that frame's physics. Optional.
///
/// `dt` is the simulated seconds since the previous call — it grows
/// with the timescale. The heavy-handed alternative to events:
/// replan everything, every frame.
fn update(dt: f64, elevators: Vec<Elevator>, floors: Vec<Floor>);

// Any exception thrown from your code — in `init`, `update`, or a
// handler — pauses the game and shows the error under the editor.
//
// ── Rhai vs Rust, in thirty seconds ─────────────────────────────
//
// Your program is Rhai, which reads like Rust with the types left
// out. Three things bite ported JavaScript and Rust intuition alike:
//
// Loop captures: a `for` loop shares ONE loop variable across
// iterations. Closures made in the body must capture a shadow:
//
//     for elevator in elevators {
//         let elevator = elevator;      // fresh binding, each turn
//         elevator.on("idle", || elevator.go_to_floor(0));
//     }
//
// Shared state: variables captured by several closures are shared
// between them — declare request lists in `init`, capture them in
// every handler.
//
// Top-level statements run once, before `init`, and functions cannot
// see top-level variables: cross-handler state lives in captures.
//
// ── Determinism ─────────────────────────────────────────────────
//
// Seeded RNG, fixed-timestep playback: same seed, same program,
// same result. The Seed readout in the stats bar names the run.
