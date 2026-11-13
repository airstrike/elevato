//! The fixed-step runner for native [`Controller`] implementations. It follows
//! the dispatch contract in [`crate::world`].
//!
//! Each frame contains a whole number of [`DT_MAX`] substeps, so runs do not
//! depend on wall-clock timing. The controller's `update` runs once per frame
//! with the full frame dt. Events are dispatched before and after each
//! substep's arrival processing. A decided challenge ends the run immediately,
//! even within a frame.

use crate::World;
use crate::challenge::Outcome;
use crate::controller::Controller;
use crate::stats::Stats;
use crate::world::DT_MAX;

/// Hard cap on drain rounds per dispatch point. Handlers may command the world
/// and thereby raise fresh events, so each dispatch drains until quiescent; a
/// controller that cascades past this many rounds has its remaining events
/// deferred to the next dispatch point instead of wedging the frame.
const DISPATCH_ROUNDS: usize = 128;

/// What a finished (or exhausted) run left behind.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Report {
    /// Final statistics snapshot.
    pub stats: Stats,
    /// [`Outcome::Running`] when the frame budget ran out before the challenge
    /// decided.
    pub outcome: Outcome,
}

/// Drives `world` with `controller` for at most `frames` frames of
/// `substeps_per_frame` × [`DT_MAX`] simulated seconds each, breaking as soon
/// as the world ends. `substeps_per_frame` is the integer timescale (a live
/// playback frame at timescale 2 is `run(…, ticks, 2)`); zero advances nothing.
pub fn run(
    world: &mut World,
    controller: &mut dyn Controller,
    frames: usize,
    substeps_per_frame: usize,
) -> Report {
    // Controllers start with the whole bank available and receive each car's
    // initial Idle after init returns.
    while world.add_elevator().is_some() {}
    controller.init(world);
    dispatch(world, controller);

    let frame_dt = substeps_per_frame as f64 * DT_MAX;
    'frames: for _ in 0..frames {
        if world.ended() {
            break;
        }
        controller.update(world, frame_dt);
        for _ in 0..substeps_per_frame {
            if world.ended() {
                break 'frames;
            }
            world.step(DT_MAX);
            dispatch(world, controller);
            world.process_arrivals();
            dispatch(world, controller);
        }
    }

    Report {
        stats: *world.stats(),
        outcome: world.outcome(),
    }
}

/// Drains the event queue to the controller until quiescent (capped at
/// [`DISPATCH_ROUNDS`]).
fn dispatch(world: &mut World, controller: &mut dyn Controller) {
    for _ in 0..DISPATCH_ROUNDS {
        let events = world.drain_events();
        if events.is_empty() {
            return;
        }
        for event in events {
            controller.on_event(world, event);
        }
    }
}
