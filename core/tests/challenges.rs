//! End-to-end challenge runs through the headless runner, driven by a
//! native port of the naive community solution (research §5.1). The pair
//! of assertions is deliberate: a sim that lets the naive strategy clear
//! challenge 5 is as broken as one that fails it challenge 1.

use elevato_core::challenge::{self, Outcome};
use elevato_core::controller::Controller;
use elevato_core::event::Event;
use elevato_core::{World, headless};

/// Research §5.1 verbatim: every elevator serves its own pressed buttons
/// directly, idles back to floor 0, and every floor call is blindly given
/// to elevator 0. No direction awareness, no load awareness.
struct Naive;

impl Controller for Naive {
    fn init(&mut self, _world: &mut World) {}

    fn on_event(&mut self, world: &mut World, event: Event) {
        match event {
            Event::Idle { elevator } => world.go_to_floor(elevator, 0.0, false),
            Event::FloorButtonPressed { elevator, floor } => {
                world.go_to_floor(elevator, floor as f64, false)
            }
            Event::UpButtonPressed { floor } | Event::DownButtonPressed { floor } => {
                world.go_to_floor(0, floor as f64, false)
            }
            Event::PassingFloor { .. } | Event::StoppedAtFloor { .. } => {}
        }
    }
}

const SEED: u64 = 1;

#[test]
fn the_naive_strategy_passes_challenge_one() {
    let mut world = World::new(&challenge::roster()[0], SEED);
    let report = headless::run(&mut world, &mut Naive, 3700, 1);
    assert_eq!(
        report.outcome,
        Outcome::Succeeded,
        "final stats: {:?}",
        report.stats
    );
    assert!(report.stats.transported() >= 15);
}

#[test]
fn the_naive_strategy_fails_challenge_five() {
    let mut world = World::new(&challenge::roster()[4], SEED);
    let report = headless::run(&mut world, &mut Naive, 4200, 1);
    assert_eq!(
        report.outcome,
        Outcome::Failed,
        "final stats: {:?}",
        report.stats
    );
    assert!(report.stats.transported() < 100);
}

#[test]
fn identical_seeds_and_controllers_replay_byte_identical_stats() {
    let run = || {
        let mut world = World::new(&challenge::roster()[1], 77);
        headless::run(&mut world, &mut Naive, 3700, 1)
    };
    let (first, second) = (run(), run());
    assert_eq!(first, second);
    assert_eq!(first.stats.transported(), second.stats.transported());
}
