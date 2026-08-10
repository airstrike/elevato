//! The fixed-timestep determinism gate (Phase 5 exit criterion): a live
//! playback of N ticks produces byte-identical stats to the same frames
//! driven directly through the script runtime, including across the
//! challenge's mid-run decision.

use elevato::playback::{self, Playback};
use script::{Program, Runtime};

#[test]
fn a_live_playback_of_n_ticks_equals_the_same_frames_driven_directly() {
    let mut playback = Playback::new(playback::STARTER).unwrap();
    playback.toggle();
    assert!(playback.is_running());

    let program = Program::compile(playback::STARTER).unwrap();
    let roster = elevato::core::challenge::roster();
    let mut runtime = Runtime::new(program, &roster[0], playback.seed()).unwrap();

    // 2000 ticks × timescale 2 substeps ≈ 66.7 simulated seconds -
    // comfortably past challenge 1's 60 s decision, so the run also
    // proves both sides freeze identically once the challenge ends.
    let timescale = playback.timescale();
    for _ in 0..2000 {
        playback.tick();
        runtime.frame(timescale).unwrap();
    }

    assert_eq!(playback.stats(), runtime.stats());
    assert_eq!(playback.outcome(), runtime.outcome());
    assert!(playback.ended(), "challenge 1 decides within 60 s");
    assert!(!playback.is_running(), "playback pauses itself at the end");
}

#[test]
fn the_parity_holds_at_a_faster_timescale_too() {
    let mut playback = Playback::new(playback::STARTER).unwrap();
    playback.speed_up();
    playback.speed_up();
    assert_eq!(playback.timescale(), 5);
    playback.toggle();

    let program = Program::compile(playback::STARTER).unwrap();
    let roster = elevato::core::challenge::roster();
    let mut runtime = Runtime::new(program, &roster[0], playback.seed()).unwrap();

    for _ in 0..300 {
        playback.tick();
        runtime.frame(5).unwrap();
    }

    assert_eq!(playback.stats(), runtime.stats());
    assert!(
        playback.stats().elapsed() > 0.0,
        "the run actually advanced"
    );
}
