//! Live playback and a directly driven [`Simulation`] of the same program
//! produce identical statistics, including when a challenge ends mid-frame.

#[path = "solutions/starter.rs"]
#[allow(dead_code)]
mod starter;

use elevato::core::challenge;
use elevato::playback::{Playback, Program, Run};

fn starter() -> Program {
    Program::new(
        starter::Program::new,
        starter::Program::update,
        starter::Program::view,
        starter::Program::subscription,
    )
}

/// The starter on challenge 1, driven directly.
fn direct(seed: u64) -> Box<dyn Run> {
    starter().boot(&challenge::roster()[0], seed).unwrap()
}

#[test]
fn a_live_playback_of_n_ticks_equals_the_same_frames_driven_directly() {
    let mut playback = Playback::new(starter());
    playback.toggle();
    assert!(playback.is_running());

    let mut run = direct(playback.seed());

    // 2000 ticks × timescale 2 substeps ≈ 66.7 simulated seconds -
    // comfortably past challenge 1's 60 s decision, so the run also
    // proves both sides freeze identically once the challenge ends.
    let timescale = playback.timescale();
    for _ in 0..2000 {
        playback.tick();
        run.frame(timescale).unwrap();
    }

    assert_eq!(playback.stats(), run.stats());
    assert_eq!(playback.outcome(), run.outcome());
    assert!(playback.ended(), "challenge 1 decides within 60 s");
    assert!(!playback.is_running(), "playback pauses itself at the end");
}

#[test]
fn the_parity_holds_at_a_faster_timescale_too() {
    let mut playback = Playback::new(starter());
    playback.speed_up();
    playback.speed_up();
    assert_eq!(playback.timescale(), 5);
    playback.toggle();

    let mut run = direct(playback.seed());

    for _ in 0..300 {
        playback.tick();
        run.frame(5).unwrap();
    }

    assert_eq!(playback.stats(), run.stats());
    assert!(
        playback.stats().elapsed() > 0.0,
        "the run actually advanced"
    );
}
