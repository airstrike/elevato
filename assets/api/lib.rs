//! The scripting surface. A program defines `init` and, optionally,
//! `update`; both receive live handles into the running world.

/// Runs once, at challenge start. Required.
fn init(elevators: Vec<Elevator>, floors: Vec<Floor>);

/// Runs once per frame, before that frame's physics. Optional.
/// `dt` is the frame's simulated seconds; it grows with the timescale.
fn update(dt: f64, elevators: Vec<Elevator>, floors: Vec<Floor>);

/// The message dialect's boot. Defining `model` — instead of `init` —
/// makes its return value the program's state and switches event
/// delivery to `update`. Also accepted: `model(elevators, floors)`.
fn model() -> Model;

/// Message form, required alongside `model`. Bound to the state as
/// `this`; mutations persist. `message` is a map: `kind` holds an
/// `Event` name in snake_case plus that event's fields — elevator
/// events carry `elevator`, floor events carry `floor` (handles,
/// commandable in place) — and time arrives as `#{ kind: "tick", dt }`.
/// `on` is unavailable in this dialect.
fn update(message, elevators, floors);

// Exceptions thrown anywhere in user code pause the run; the message
// surfaces under the editor.
//
// Rhai shares one loop variable across `for` iterations, so closures
// bound in the body must capture a per-iteration shadow:
//
//     for elevator in elevators {
//         let elevator = elevator;
//         elevator.on("idle", || elevator.go_to_floor(0));
//     }
//
// Variables captured by several closures are shared between them.
// Top-level statements run once, before `init`, and functions cannot
// see top-level variables — cross-handler state lives in captures.
//
// Runs are deterministic: same seed, same program, same result. The
// stats bar's Seed names the attempt.
