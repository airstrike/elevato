//! Trajectory-level integration tests. These act as the driver: they own
//! the substep + dispatch + `process_arrivals` loop that the script
//! runtime and headless runner will own in later phases.

use elevato_core::World;
use elevato_core::elevator::{ACCELERATION, DECELERATION, MAXSPEED};
use elevato_core::event::{Direction, Event};
use elevato_core::floor;
use elevato_core::world::DT_MAX;

/// One driver iteration: step, drain, process arrivals, drain again.
/// Returns all events with the game time at which they were drained.
fn substep(world: &mut World, t: f64) -> Vec<(f64, Event)> {
    world.step(DT_MAX);
    let mut drained: Vec<(f64, Event)> = world
        .drain_events()
        .into_iter()
        .map(|event| (t, event))
        .collect();
    world.process_arrivals();
    drained.extend(world.drain_events().into_iter().map(|event| (t, event)));
    drained
}

/// Drives the world until an arrival (or `max_substeps` runs out),
/// returning `(events, samples)` where samples are per-substep
/// `(t, y, velocity_y)` readings taken after each step.
fn drive_until_arrival(
    world: &mut World,
    max_substeps: usize,
) -> (Vec<(f64, Event)>, Vec<(f64, f64, f64)>) {
    let mut events = Vec::new();
    let mut samples = Vec::new();
    for n in 1..=max_substeps {
        let t = n as f64 * DT_MAX;
        let drained = substep(world, t);
        let arrived = drained
            .iter()
            .any(|(_, event)| matches!(event, Event::StoppedAtFloor { .. }));
        events.extend(drained);
        let elevator = &world.elevators()[0];
        samples.push((t, elevator.y(), elevator.velocity_y()));
        if arrived {
            break;
        }
    }
    (events, samples)
}

#[test]
fn a_zero_to_three_trip_matches_analytic_kinematics_with_three_moves_and_a_one_second_dwell() {
    let mut world = World::new(4, 1).unwrap();
    world.go_to_floor(0, 3.0, false);
    let start_y = world.elevators()[0].y();
    assert_eq!(start_y, 150.0);

    let (events, samples) = drive_until_arrival(&mut world, 6 * 60);

    // Arrival: snapped exactly onto floor 3, three floor boundaries crossed,
    // dwell running.
    let elevator = &world.elevators()[0];
    assert_eq!(elevator.y(), floor::y_of_level(3.0, 4));
    assert_eq!(elevator.velocity_y(), 0.0);
    assert_eq!(elevator.move_count(), 3);
    assert!(elevator.is_busy(), "arrival must start the door dwell");
    assert!(events.iter().any(|(_, event)| *event
        == Event::StoppedAtFloor {
            elevator: 0,
            floor: 3
        }));

    // Acceleration phase: top speed is reached at t = MAXSPEED/ACCELERATION
    // (1.238 s analytically), within two substeps of Euler slack.
    let (t_top, y_top, _) = *samples
        .iter()
        .find(|(_, _, v)| v.abs() >= MAXSPEED)
        .expect("the trip must reach top speed");
    let t_top_analytic = MAXSPEED / ACCELERATION;
    assert!(
        (t_top - t_top_analytic).abs() <= 2.5 * DT_MAX,
        "top speed at {t_top:.4} s, analytic {t_top_analytic:.4} s"
    );
    // Distance covered while accelerating: v²/(2a) = 80.5 px analytically.
    let accel_distance = start_y - y_top;
    let accel_distance_analytic = MAXSPEED * MAXSPEED / (2.0 * ACCELERATION);
    assert!(
        (accel_distance - accel_distance_analytic).abs() < 3.0,
        "accelerated over {accel_distance:.2} px, analytic {accel_distance_analytic:.2} px"
    );

    // Braking phase: engages when 1.05 × braking distance (68.25 px at top
    // speed) first covers the remaining distance.
    let braking_index = samples
        .windows(2)
        .position(|pair| pair[1].2.abs() < pair[0].2.abs())
        .expect("the trip must brake");
    let remaining_at_braking = samples[braking_index].1 - floor::y_of_level(3.0, 4);
    let engage_analytic = 1.05 * MAXSPEED * MAXSPEED / (2.0 * DECELERATION);
    assert!(
        (remaining_at_braking - engage_analytic).abs() < 4.0,
        "braking began with {remaining_at_braking:.2} px remaining, analytic {engage_analytic:.2} px"
    );

    // Total trip time: continuous-optimal is ~2.30 s (accelerate 1.24 s,
    // cruise ~0.01 s, brake ~1.05 s); the discrete soft-ramp tail may add a
    // little, never subtract.
    let (t_arrival, _, _) = *samples.last().unwrap();
    assert!(
        (2.2..=2.6).contains(&t_arrival),
        "trip took {t_arrival:.4} s"
    );

    // Dwell: a destination queued during the dwell departs only after
    // 1.0 s of doors-open time.
    world.go_to_floor(0, 0.0, false);
    assert!(world.elevators()[0].is_busy());
    let mut t_departure = None;
    for n in 1..=120 {
        let t = t_arrival + n as f64 * DT_MAX;
        substep(&mut world, t);
        let elevator = &world.elevators()[0];
        if elevator.velocity_y() != 0.0 {
            t_departure = Some(t);
            break;
        }
        assert_eq!(
            elevator.y(),
            floor::y_of_level(3.0, 4),
            "must hold still while dwelling"
        );
    }
    let dwell = t_departure.expect("the elevator must depart again") - t_arrival;
    assert!(
        (0.98..=1.06).contains(&dwell),
        "dwelled {dwell:.4} s before departing"
    );
}

#[test]
fn passing_floor_fires_for_intermediate_floors_only() {
    let mut world = World::new(4, 1).unwrap();
    world.go_to_floor(0, 3.0, false);

    let (events, _) = drive_until_arrival(&mut world, 6 * 60);
    let passings: Vec<(usize, Direction)> = events
        .iter()
        .filter_map(|(_, event)| match event {
            Event::PassingFloor {
                floor, direction, ..
            } => Some((*floor, *direction)),
            Event::Idle { .. }
            | Event::StoppedAtFloor { .. }
            | Event::FloorButtonPressed { .. } => None,
        })
        .collect();
    assert_eq!(passings, vec![(1, Direction::Up), (2, Direction::Up)]);
}

#[test]
fn go_to_floor_issued_while_draining_idle_takes_effect_in_the_same_iteration() {
    let mut world = World::new(4, 1).unwrap();

    // The driver fires the initial queue check (world.init in the original).
    world.check_destination_queue(0);
    let drained = world.drain_events();
    assert_eq!(drained, vec![Event::Idle { elevator: 0 }]);

    // Responding to Idle mid-drain starts movement synchronously...
    world.go_to_floor(0, 2.0, false);
    let elevator = &world.elevators()[0];
    assert!(elevator.is_moving());
    assert_eq!(elevator.destination_queue(), &[2.0]);

    // ...so the very next substep accelerates it (velocity integrates
    // into position one substep later — velocity-before-acceleration
    // Euler, research §3).
    world.step(DT_MAX);
    let elevator = &world.elevators()[0];
    assert!(elevator.velocity_y() != 0.0);
    world.step(DT_MAX);
    let elevator = &world.elevators()[0];
    assert!(elevator.y() < 150.0, "must have moved upward");
}

#[test]
fn stop_mid_flight_halts_between_floors_with_no_arrival_events_and_no_dwell() {
    let mut world = World::new(4, 1).unwrap();
    world.go_to_floor(0, 3.0, false);

    // Reach cruising speed, then stop.
    let mut t = 0.0;
    while world.elevators()[0].velocity_y().abs() < MAXSPEED {
        t += DT_MAX;
        substep(&mut world, t);
    }
    world.stop(0);
    assert!(world.elevators()[0].destination_queue().is_empty());

    // Run well past the halt: it must settle between floors, silently.
    let mut post_stop_events = Vec::new();
    for _ in 0..(3 * 60) {
        t += DT_MAX;
        post_stop_events.extend(substep(&mut world, t));
        assert!(
            !world.elevators()[0].is_busy(),
            "a stop() halt never dwells"
        );
    }
    let elevator = &world.elevators()[0];
    assert_eq!(elevator.velocity_y(), 0.0);
    assert!(!elevator.is_moving());
    let exact = elevator.exact_floor();
    assert!(
        (exact - exact.round()).abs() > 0.05,
        "halted at {exact:.4}, which is on a floor"
    );
    assert_eq!(post_stop_events, vec![], "a stop() halt emits nothing");
}

#[test]
fn duplicate_suppression_checks_only_the_adjacent_queue_element() {
    let mut world = World::new(4, 1).unwrap();

    world.go_to_floor(0, 2.0, false);
    world.go_to_floor(0, 2.0, false);
    assert_eq!(world.elevators()[0].destination_queue(), &[2.0]);

    world.go_to_floor(0, 3.0, false);
    world.go_to_floor(0, 2.0, false);
    assert_eq!(world.elevators()[0].destination_queue(), &[2.0, 3.0, 2.0]);

    // Forced entries compare against the queue front instead.
    world.go_to_floor(0, 2.0, true);
    assert_eq!(world.elevators()[0].destination_queue(), &[2.0, 3.0, 2.0]);
    world.go_to_floor(0, 1.0, true);
    assert_eq!(
        world.elevators()[0].destination_queue(),
        &[1.0, 2.0, 3.0, 2.0]
    );
}

#[test]
fn idle_fires_one_second_after_the_last_arrival() {
    let mut world = World::new(4, 1).unwrap();
    world.go_to_floor(0, 1.0, false);

    let (_, samples) = drive_until_arrival(&mut world, 6 * 60);
    let (t_arrival, _, _) = *samples.last().unwrap();

    let mut t_idle = None;
    for n in 1..=120 {
        let t = t_arrival + n as f64 * DT_MAX;
        let drained = substep(&mut world, t);
        if drained
            .iter()
            .any(|(_, event)| *event == Event::Idle { elevator: 0 })
        {
            t_idle = Some(t);
            break;
        }
    }
    let after = t_idle.expect("an empty queue must go idle after the dwell") - t_arrival;
    assert!((0.98..=1.06).contains(&after), "idle after {after:.4} s");
}

#[test]
fn stop_during_a_dwell_only_clears_the_queue() {
    let mut world = World::new(4, 1).unwrap();
    world.go_to_floor(0, 1.0, false);
    let (_, samples) = drive_until_arrival(&mut world, 6 * 60);
    let (t_arrival, _, _) = *samples.last().unwrap();

    world.go_to_floor(0, 3.0, false);
    assert_eq!(world.elevators()[0].destination_queue(), &[3.0]);

    world.stop(0);
    let elevator = &world.elevators()[0];
    assert!(elevator.destination_queue().is_empty());
    assert!(elevator.is_busy(), "stop() during a dwell keeps the dwell");

    // With nothing queued, the dwell ends in idleness, still on floor 1.
    let mut saw_idle = false;
    for n in 1..=120 {
        let t = t_arrival + n as f64 * DT_MAX;
        let drained = substep(&mut world, t);
        saw_idle |= drained
            .iter()
            .any(|(_, event)| *event == Event::Idle { elevator: 0 });
    }
    assert!(saw_idle);
    let elevator = &world.elevators()[0];
    assert_eq!(elevator.exact_floor(), 1.0);
    assert_eq!(elevator.velocity_y(), 0.0);
}

#[test]
fn a_forced_go_to_floor_at_the_current_floor_causes_a_re_arrival() {
    let mut world = World::new(4, 1).unwrap();

    // Standing at floor 0; the world's Phase 3 re-arrival rule issues
    // exactly this command when a call button is pressed beside a parked
    // elevator.
    world.go_to_floor(0, 0.0, true);
    let events = substep(&mut world, DT_MAX);
    assert!(
        events.iter().any(|(_, event)| *event
            == Event::StoppedAtFloor {
                elevator: 0,
                floor: 0
            }),
        "re-arrival must re-fire StoppedAtFloor"
    );
    assert!(world.elevators()[0].is_busy(), "re-arrival dwells again");
}

#[test]
fn pressing_an_in_elevator_button_emits_once_and_arrival_clears_it() {
    let mut world = World::new(4, 1).unwrap();

    world.press_floor_button(0, 2);
    world.press_floor_button(0, 2);
    assert_eq!(
        world.drain_events(),
        vec![Event::FloorButtonPressed {
            elevator: 0,
            floor: 2
        }]
    );
    assert_eq!(world.elevators()[0].pressed_floors(), vec![2]);

    world.go_to_floor(0, 2.0, false);
    drive_until_arrival(&mut world, 6 * 60);
    assert_eq!(world.elevators()[0].pressed_floors(), Vec::<usize>::new());
}
