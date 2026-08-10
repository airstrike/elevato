//! End-to-end proof that ported community solutions drive real
//! challenges through the rhai runtime exactly as they did in the
//! original - asserting known failures as well as known passes - plus
//! the parity pin: the rhai naive solution must produce byte-identical
//! final stats to its native-controller twin run through
//! `core::headless`, freezing the runtime's replicated driver loop to
//! the core contract.

use elevato_core::challenge::{self, Outcome};
use elevato_core::controller::Controller;
use elevato_core::event::Event;
use elevato_core::{World, headless};
use script::{Error, Program, Runtime};

const STARTER: &str = include_str!("solutions/starter.rhai");
const NAIVE: &str = include_str!("solutions/naive.rhai");
const INDICATOR: &str = include_str!("solutions/indicator.rhai");
const TWENTYLINER: &str = include_str!("solutions/twentyliner.rhai");

const SEED: u64 = 1;

/// Compiles `source` and drives challenge `index` (0-based) for at most
/// `frames` single-substep frames, breaking when the challenge decides -
/// the same budgeting as `headless::run(…, frames, 1)`.
fn run(source: &str, index: usize, seed: u64, frames: usize) -> Runtime {
    let program = Program::compile(source).expect("solution must compile");
    let mut runtime =
        Runtime::new(program, &challenge::roster()[index], seed).expect("boot must run cleanly");
    for _ in 0..frames {
        if runtime.ended() {
            break;
        }
        runtime.frame(1).expect("no runtime error expected");
    }
    runtime
}

/// Compiles `source` on challenge `index` and drives it until it
/// errors, up to `frames`.
fn run_until_error(source: &str, index: usize, frames: usize) -> Error {
    let program = Program::compile(source).expect("source must compile");
    let mut runtime =
        Runtime::new(program, &challenge::roster()[index], SEED).expect("boot must run cleanly");
    for _ in 0..frames {
        if let Err(error) = runtime.frame(1) {
            return error;
        }
    }
    panic!("the script never raised the expected runtime error");
}

#[test]
fn the_starter_program_runs_challenge_one_without_script_errors() {
    let runtime = run(STARTER, 0, SEED, 3700);
    // Shuttling floors 0 and 1 cannot clear a 3-floor challenge; the
    // point is a clean run to the time limit, moving at least somebody.
    assert_eq!(runtime.outcome(), Outcome::Failed);
    assert!(runtime.stats().transported() > 0);
}

#[test]
fn the_naive_port_passes_challenge_one() {
    let runtime = run(NAIVE, 0, SEED, 3700);
    assert_eq!(
        runtime.outcome(),
        Outcome::Succeeded,
        "final stats: {:?}",
        runtime.stats()
    );
    assert!(runtime.stats().transported() >= 15);
}

#[test]
fn the_naive_port_fails_challenge_five() {
    let runtime = run(NAIVE, 4, SEED, 4200);
    assert_eq!(
        runtime.outcome(),
        Outcome::Failed,
        "final stats: {:?}",
        runtime.stats()
    );
    assert!(runtime.stats().transported() < 100);
}

/// The native twin of `naive.rhai` (cribbed from core's own challenge
/// tests) - issuing exactly the same commands at exactly the same
/// dispatch points.
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

#[test]
fn the_rhai_naive_replays_the_native_naive_byte_for_byte() {
    let mut world = World::new(&challenge::roster()[0], SEED);
    let native = headless::run(&mut world, &mut Naive, 3700, 1);

    let scripted = run(NAIVE, 0, SEED, 3700);

    assert_eq!(scripted.stats(), native.stats, "stats diverged");
    assert_eq!(scripted.outcome(), native.outcome);
}

#[test]
fn the_indicator_port_passes_challenge_four_where_the_naive_fails() {
    // Challenge 4 (8 floors, 2 elevators) is where request routing and
    // indicator-steered sweeps first beat blind dispatch: across seeds
    // 1-10 the naive fails 9 times while the indicator port passes 8.
    // Challenge 5 was tried first per the plan and is genuinely out of
    // this strategy's reach - it is a raw-throughput lobby shuttle
    // (6 floors, 4 cars, spawn 1.7/s) where direction-filtered boarding
    // costs more than it saves; the indicator port peaks at 98/100 over
    // ten seeds (and the naive actually edges it out there).
    const CHALLENGE_FOUR: usize = 3;
    const DECISIVE_SEED: u64 = 2;

    let naive = run(NAIVE, CHALLENGE_FOUR, DECISIVE_SEED, 3700);
    assert_eq!(
        naive.outcome(),
        Outcome::Failed,
        "stats: {:?}",
        naive.stats()
    );

    let indicator = run(INDICATOR, CHALLENGE_FOUR, DECISIVE_SEED, 3700);
    assert_eq!(
        indicator.outcome(),
        Outcome::Succeeded,
        "final stats: {:?}",
        indicator.stats()
    );
}

#[test]
fn the_twentyliner_port_clears_challenges_one_through_four_and_six() {
    for (index, frames) in [(0, 3700), (1, 3700), (2, 3700), (3, 3700), (5, 30000)] {
        let runtime = run(TWENTYLINER, index, SEED, frames);
        assert_eq!(
            runtime.outcome(),
            Outcome::Succeeded,
            "challenge {} - final stats: {:?}",
            index + 1,
            runtime.stats()
        );
    }
}

#[test]
fn a_program_without_new_is_a_compile_time_error() {
    let error = Program::compile("fn update(message, elevators, floors) {}").unwrap_err();
    assert!(matches!(error, Error::MissingNew));
}

#[test]
fn a_new_with_parameters_is_a_compile_time_error() {
    let error = Program::compile("fn new(a) { #{} }\nfn update(message, elevators, floors) {}")
        .unwrap_err();
    assert!(matches!(error, Error::NewArity(1)));
}

#[test]
fn a_program_without_update_is_a_compile_time_error() {
    let error = Program::compile("fn new() { #{} }").unwrap_err();
    assert!(matches!(error, Error::MissingUpdate));
}

#[test]
fn an_update_returning_a_non_command_is_a_runtime_error_naming_the_type() {
    // The initial idle round delivers the first message during
    // construction, so the bad return surfaces from `Runtime::new`.
    let program =
        Program::compile("fn new() { #{} }\nfn update(message, elevators, floors) { 42 }").unwrap();
    let error = Runtime::new(program, &challenge::roster()[0], SEED).unwrap_err();
    assert!(matches!(error, Error::Runtime(_)));
    assert!(
        error.to_string().contains("i64"),
        "unexpected error display: {error}"
    );
}

#[test]
fn a_command_array_with_a_non_command_element_is_a_runtime_error() {
    let source = r#"
fn new() { #{} }
fn update(message, elevators, floors) {
    [go_to_floor(0, 1), "nope"]
}
"#;
    let program = Program::compile(source).unwrap();
    let error = Runtime::new(program, &challenge::roster()[0], SEED).unwrap_err();
    assert!(
        error.to_string().contains("string"),
        "unexpected error display: {error}"
    );
}

#[test]
fn a_command_for_an_elevator_the_challenge_does_not_have_is_a_runtime_error() {
    // Challenge 1 has a single elevator; index 1 does not exist. The
    // index is refused, never clamped.
    let source = r#"
fn new() { #{} }
fn update(message, elevators, floors) {
    switch message {
        Message::Idle(_) => go_to_floor(1, 0)
    }
}
"#;
    let program = Program::compile(source).unwrap();
    let error = Runtime::new(program, &challenge::roster()[0], SEED).unwrap_err();
    let display = error.to_string();
    assert!(
        display.contains("go_to_floor") && display.contains("no elevator 1"),
        "unexpected error display: {display}"
    );
}

#[test]
fn a_throwing_update_surfaces_a_runtime_error_with_its_position() {
    let source = r#"
fn new() {
    #{}
}

fn update(message, elevators, floors) {
    switch message {
        Message::StoppedAtFloor(_, _) => { throw "kaboom"; }
        Message::Idle(elevator) => go_to_floor(elevator, 1)
    }
}
"#;
    let error = run_until_error(source, 0, 600);
    assert!(matches!(error, Error::Runtime(_)));
    let display = error.to_string();
    assert!(
        display.contains("kaboom") && display.contains("line 8"),
        "unexpected error display: {display}"
    );
}

#[test]
fn the_tier_one_snapshot_fields_are_readable_from_scripts() {
    // Every elevator and floor snapshot field, probed on the first tick
    // (fresh, parked world) - a snapshot-builder typo throws here.
    let source = r#"
fn new() {
    #{ checked: false }
}

fn update(message, elevators, floors) {
    switch message {
        Message::Tick(_) => {
            if this.checked { return; }
            this.checked = true;
            probe(elevators, floors)
        }
    }
}

fn probe(elevators, floors) {
    let e = elevators[0];
    if e.current_floor != 0 { throw "fresh elevator away from the lobby"; }
    if e.max_passenger_count != 4 { throw "challenge 1 capacity is 4"; }
    if e.load_factor != 0.0 { throw "empty elevator carries weight"; }
    if e.is_full { throw "empty elevator claims to be full"; }
    if e.destination_direction != "stopped" { throw "parked elevator has a direction"; }
    if !e.destination_queue.is_empty() { throw "fresh elevator has a queue"; }
    if !e.pressed_floors.is_empty() { throw "fresh elevator has lit buttons"; }
    if e.move_count != 0 { throw "fresh elevator has moves"; }
    if e.is_busy { throw "parked elevator claims to be dwelling"; }
    if e.is_moving { throw "parked elevator claims to be moving"; }
    if !e.is_on_a_floor { throw "parked elevator floats between floors"; }
    if !e.going_up_indicator { throw "the up lamp starts on"; }
    if !e.going_down_indicator { throw "the down lamp starts on"; }
    let f = floors[1];
    if f.floor_num != 1 { throw "floor 1 misnumbered"; }
    if f.level != f.floor_num { throw "level must alias floor_num"; }
    // The first tick precedes the first physics step: nobody has
    // spawned, so no call button is lit yet.
    if f.up_pressed { throw "fresh floor has a lit up button"; }
    if f.down_pressed { throw "fresh floor has a lit down button"; }
    go_to_floor(0, 2)
}
"#;
    let runtime = run(source, 0, SEED, 600);
    assert!(
        runtime.world().elevators()[0].move_count() >= 2,
        "the commanded trip must be visible through move_count"
    );
}

#[test]
fn model_state_persists_across_update_calls() {
    let source = r#"
fn new() {
    #{ launched: false }
}

fn update(message, elevators, floors) {
    switch message {
        Message::Tick(_) => {
            if !this.launched {
                this.launched = true;
                return go_to_floor(0, 2);
            }
        }
    }
}
"#;
    let runtime = run(source, 0, SEED, 600);
    let world = runtime.world();
    assert_eq!(world.elevators()[0].current_floor(), 2);
    assert_eq!(
        world.elevators()[0].move_count(),
        2,
        "exactly one trip: the launched flag must persist"
    );
}
