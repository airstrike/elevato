//! Ported community solutions, compiled as ordinary Rust against the API
//! and run headlessly through a [`Simulation`], asserting known failures
//! as well as known passes - plus the parity pins: each solution's final
//! stats on a fixed challenge and seed, freezing the simulation's
//! delivery order and the solutions' behaviour together.
//!
//! The behaviour tests at the end drive the API with small programs
//! written as closures.

// A program's messages carry more than each solution reads, and the
// starter's `main` opens the game, which no test does.
#[path = "solutions/indicator.rs"]
#[allow(dead_code)]
mod indicator;
#[path = "solutions/naive.rs"]
#[allow(dead_code)]
mod naive;
#[path = "solutions/starter.rs"]
#[allow(dead_code)]
mod starter;
#[path = "solutions/twentyliner.rs"]
#[allow(dead_code)]
mod twentyliner;

use elevato::core::challenge::{self, Outcome};
use elevato::core::stats::Stats;
use elevato::{Direction, Elevator, elevator, floor};
use elevato_api::Simulation;
use elevato_api::simulation::Error;
use iced::{Subscription, Task};

const SEED: u64 = 1;

/// Boots `$solution` on challenge `$challenge` (0-based) and drives it
/// for at most `$frames` single-substep frames, breaking when the
/// challenge decides.
macro_rules! run {
    ($solution:ident, $challenge:expr, $seed:expr, $frames:expr) => {{
        let mut simulation = Simulation::new(
            &challenge::roster()[$challenge],
            $seed,
            $solution::Program::new,
            $solution::Program::update,
            $solution::Program::subscription,
        )
        .expect("the program boots");
        for _ in 0..$frames {
            if simulation.ended() {
                break;
            }
            simulation.frame(1).expect("the program runs");
        }
        simulation
    }};
}

/// The pinned figures of a run: transported, moves, and the elapsed and
/// wait times in whole milliseconds.
fn pin(stats: Stats) -> (usize, usize, u64, u64, u64) {
    let millis = |seconds: f64| (seconds * 1000.0).round() as u64;
    (
        stats.transported(),
        stats.move_count(),
        millis(stats.elapsed()),
        millis(stats.max_wait_time()),
        millis(stats.avg_wait_time()),
    )
}

#[test]
fn the_starter_runs_challenge_one_to_its_time_limit() {
    let run = run!(starter, 0, SEED, 3700);
    // Shuttling floors 0 and 1 cannot clear a 3-floor challenge; the
    // point is a clean run to the time limit, moving somebody.
    assert_eq!(run.outcome(), Outcome::Failed);
    assert_eq!(pin(run.stats()), (3, 26, 60017, 56683, 3656));
}

#[test]
fn the_naive_port_passes_challenge_one() {
    let run = run!(naive, 0, SEED, 3700);
    assert_eq!(run.outcome(), Outcome::Succeeded);
    assert_eq!(pin(run.stats()), (15, 28, 51167, 10400, 6126));
}

#[test]
fn the_naive_port_fails_challenge_five() {
    let run = run!(naive, 4, SEED, 4200);
    assert_eq!(run.outcome(), Outcome::Failed);
    assert_eq!(pin(run.stats()), (97, 182, 68017, 22833, 9012));
}

#[test]
fn the_indicator_port_passes_challenge_four_where_the_naive_fails() {
    // Challenge 4 (8 floors, 2 elevators) is where routing calls and
    // steering boarding with the lamps first beat blind dispatch. Over
    // seeds 1 to 10 the indicator port passes 7 and the naive 4; seed
    // 6 is decisive both ways.
    const CHALLENGE_FOUR: usize = 3;
    const DECISIVE_SEED: u64 = 6;

    let naive = run!(naive, CHALLENGE_FOUR, DECISIVE_SEED, 3700);
    assert_eq!(naive.outcome(), Outcome::Failed, "{:?}", naive.stats());

    let indicator = run!(indicator, CHALLENGE_FOUR, DECISIVE_SEED, 3700);
    assert_eq!(
        indicator.outcome(),
        Outcome::Succeeded,
        "{:?}",
        indicator.stats()
    );
    assert_eq!(pin(indicator.stats()), (28, 92, 52650, 22650, 10479));
}

#[test]
fn the_twentyliner_port_clears_challenges_one_through_four_and_six() {
    for (index, frames) in [(0, 3700), (1, 3700), (2, 3700), (3, 3700), (5, 30000)] {
        let run = run!(twentyliner, index, SEED, frames);
        assert_eq!(
            run.outcome(),
            Outcome::Succeeded,
            "challenge {}: {:?}",
            index + 1,
            run.stats()
        );
    }
    let run = run!(twentyliner, 3, SEED, 3700);
    assert_eq!(pin(run.stats()), (30, 84, 59950, 23283, 9510));
}

/// What a car told a test program, in order.
type Log = Vec<(Elevator, elevator::Message)>;

/// A program that runs `start` once and records every car message.
fn recorder(
    challenge: usize,
    start: impl FnOnce(&[Elevator]) -> Task<elevator::Message>,
) -> Simulation<Log, (Elevator, elevator::Message)> {
    Simulation::new(
        &challenge::roster()[challenge],
        SEED,
        |elevators: Vec<Elevator>, _floors: usize| {
            let car = elevators[0];
            (
                Log::new(),
                start(&elevators).map(move |message| (car, message)),
            )
        },
        |log: &mut Log, entry: (Elevator, elevator::Message)| {
            log.push(entry);
            Task::none()
        },
        |_: &Log| Subscription::none(),
    )
    .expect("the program boots")
}

/// Drives `simulation` for `frames` single-substep frames.
fn drive<P, M: Send + 'static>(simulation: &mut Simulation<P, M>, frames: usize) {
    for _ in 0..frames {
        simulation.frame(1).expect("the program runs");
    }
}

#[test]
fn a_trip_streams_the_floors_it_passes_and_ends_where_it_stops() {
    let mut simulation = recorder(0, |elevators| elevators[0].go_to_floor(2));
    drive(&mut simulation, 600);
    let messages: Vec<elevator::Message> = simulation_log(&simulation);
    assert_eq!(
        messages.first(),
        Some(&elevator::Message::PassingFloor(1, Direction::Up))
    );
    assert!(matches!(
        messages.last(),
        Some(elevator::Message::StoppedAtFloor { floor, .. }) if *floor == 2
    ));
    assert_eq!(simulation.world().elevators()[0].current_floor(), 2);
}

#[test]
fn a_floor_past_the_top_is_the_top_floor() {
    let mut simulation = recorder(0, |elevators| elevators[0].go_to_floor(99));
    drive(&mut simulation, 600);
    assert!(matches!(
        simulation_log(&simulation).last(),
        Some(elevator::Message::StoppedAtFloor { floor, .. }) if *floor == 2
    ));
}

#[test]
fn asking_twice_for_the_last_queued_floor_ends_both_trips_at_one_stop() {
    let mut simulation = recorder(0, |elevators| {
        Task::batch([elevators[0].go_to_floor(2), elevators[0].go_to_floor(2)])
    });
    drive(&mut simulation, 600);
    let stops = simulation_log(&simulation)
        .into_iter()
        .filter(|message| matches!(message, elevator::Message::StoppedAtFloor { .. }))
        .count();
    assert_eq!(stops, 2);
    assert_eq!(simulation.world().elevators()[0].move_count(), 2);
}

#[test]
fn going_to_a_floor_first_from_passing_floor_stops_there() {
    // Sent to floor 2, the car hears it is passing floor 1 and goes
    // there first; the trip to 2 resumes after.
    let mut simulation = Simulation::new(
        &challenge::roster()[0],
        SEED,
        |elevators: Vec<Elevator>, _: usize| {
            let car = elevators[0];
            (
                Log::new(),
                car.go_to_floor(2).map(move |message| (car, message)),
            )
        },
        |log: &mut Log, (car, message): (Elevator, elevator::Message)| {
            log.push((car, message));
            match message {
                elevator::Message::PassingFloor(floor, _) => car
                    .go_to_floor_first(floor)
                    .map(move |message| (car, message)),
                elevator::Message::FloorButtonPressed(_)
                | elevator::Message::StoppedAtFloor { .. } => Task::none(),
            }
        },
        |_: &Log| Subscription::none(),
    )
    .expect("the program boots");
    drive(&mut simulation, 900);
    let stops: Vec<usize> = simulation_log(&simulation)
        .into_iter()
        .filter_map(|message| match message {
            elevator::Message::StoppedAtFloor { floor, .. } => Some(floor),
            elevator::Message::FloorButtonPressed(_) | elevator::Message::PassingFloor(..) => None,
        })
        .collect();
    assert_eq!(stops, vec![1, 2]);
    assert_eq!(simulation.world().elevators()[0].move_count(), 2);
}

#[test]
fn stop_aborts_the_trip_it_interrupts() {
    // The program stops the car as soon as it hears it passing floor 1.
    let mut stopping = Simulation::new(
        &challenge::roster()[0],
        SEED,
        |elevators: Vec<Elevator>, _: usize| {
            let car = elevators[0];
            (
                (car, Log::new()),
                car.go_to_floor(2).map(move |message| (car, message)),
            )
        },
        |(car, log): &mut (Elevator, Log), entry: (Elevator, elevator::Message)| {
            let passing = matches!(entry.1, elevator::Message::PassingFloor(..));
            log.push(entry);
            if passing {
                let car = *car;
                car.stop().map(move |message| (car, message))
            } else {
                Task::none()
            }
        },
        |_: &(Elevator, Log)| Subscription::none(),
    )
    .expect("the program boots");
    drive(&mut stopping, 600);
    let car = &stopping.world().elevators()[0];
    assert!(!car.is_on_a_floor(), "the car halts between floors");
    assert!(car.destination_queue().is_empty());
}

#[test]
fn indicator_commands_reach_the_car() {
    let mut simulation = recorder(0, |elevators| {
        Task::batch([
            elevators[0].set_going_up_indicator(false),
            elevators[0].set_going_down_indicator(true),
        ])
    });
    drive(&mut simulation, 1);
    let car = &simulation.world().elevators()[0];
    assert!(!car.going_up_indicator());
    assert!(car.going_down_indicator());
}

#[test]
fn a_task_does_nothing_until_the_program_returns_it() {
    let mut simulation = recorder(0, |elevators| {
        let _forgotten = elevators[0].go_to_floor(2);
        Task::none()
    });
    drive(&mut simulation, 600);
    assert_eq!(simulation.world().elevators()[0].move_count(), 0);
}

#[test]
fn riders_call_cars_and_press_their_buttons() {
    #[derive(Debug, Clone)]
    enum Call {
        Floor(usize, floor::Message),
        Elevator(Elevator, elevator::Message),
    }

    let mut simulation = Simulation::new(
        &challenge::roster()[0],
        SEED,
        |elevators: Vec<Elevator>, _: usize| (elevators, Task::none()),
        |elevators: &mut Vec<Elevator>, call: Call| match call {
            Call::Floor(
                floor,
                floor::Message::UpButtonPressed | floor::Message::DownButtonPressed,
            ) => elevators[0].go_to_floor(floor).discard(),
            Call::Elevator(elevator, elevator::Message::FloorButtonPressed(floor)) => {
                elevator.go_to_floor(floor).discard()
            }
            Call::Elevator(..) => Task::none(),
        },
        |_: &Vec<Elevator>| elevato::riders(Call::Elevator, Call::Floor),
    )
    .expect("the program boots");
    drive(&mut simulation, 3700);
    assert!(simulation.stats().transported() > 0);
}

#[test]
fn a_panicking_program_ends_the_run_with_its_message() {
    let mut simulation = Simulation::new(
        &challenge::roster()[0],
        SEED,
        |elevators: Vec<Elevator>, _: usize| {
            let car = elevators[0];
            ((), car.go_to_floor(1))
        },
        |_: &mut (), message: elevator::Message| match message {
            elevator::Message::StoppedAtFloor { .. } => panic!("kaboom"),
            elevator::Message::FloorButtonPressed(_) | elevator::Message::PassingFloor(..) => {
                Task::none()
            }
        },
        |_: &()| Subscription::none(),
    )
    .expect("the program boots");
    let error = (0..600)
        .find_map(|_| simulation.frame(1).err())
        .expect("the program panics");
    assert!(matches!(&error, Error::Panicked(message) if message == "kaboom"));
    assert!(
        simulation.frame(1).is_err(),
        "a run that panicked stays over"
    );
}

/// The messages a [`recorder`] logged, without their car.
fn simulation_log(
    simulation: &Simulation<Log, (Elevator, elevator::Message)>,
) -> Vec<elevator::Message> {
    simulation
        .program()
        .iter()
        .map(|&(_, message)| message)
        .collect()
}
