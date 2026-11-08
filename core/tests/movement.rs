//! Trajectory-level integration tests. These act as the driver: they own
//! the substep + dispatch + `process_arrivals` loop that the script
//! runtime and headless runner will own in later phases.

use elevato_core::World;
use elevato_core::challenge::{Challenge, Condition};
use elevato_core::elevator::{ACCELERATION, DECELERATION, MAXSPEED};
use elevato_core::event::{Direction, Event};
use elevato_core::floor;
use elevato_core::world::DT_MAX;

/// A spawnless world (spawn rate 0, perpetual condition) so trajectories
/// stay free of passenger noise, with every car added and their first
/// `Idle`s drained.
fn quiet_world(floors: usize, elevators: usize) -> World {
    let challenge = Challenge::new(floors, elevators, vec![4], 0.0, Condition::Demo).unwrap();
    let mut world = World::new(&challenge, 0);
    while world.add_elevator().is_some() {}
    let _ = world.drain_events();
    world
}

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

/// One per-substep `(t, y, velocity_y)` reading.
type Sample = (f64, f64, f64);

/// Drives the world until an arrival (or `max_substeps` runs out),
/// returning `(events, samples)` where samples are per-substep
/// readings taken after each step.
fn drive_until_arrival(world: &mut World, max_substeps: usize) -> (Vec<(f64, Event)>, Vec<Sample>) {
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
    let mut world = quiet_world(4, 1);
    world.go_to_floor(0, 3.0);
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

    // Dwell: a trip started during the dwell departs only after 1.0 s of
    // doors-open time.
    world.go_to_floor(0, 0.0);
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
    let mut world = quiet_world(4, 1);
    world.go_to_floor(0, 3.0);

    let (events, _) = drive_until_arrival(&mut world, 6 * 60);
    let passings: Vec<(usize, Direction)> = events
        .iter()
        .filter_map(|(_, event)| match event {
            Event::PassingFloor {
                floor, direction, ..
            } => Some((*floor, *direction)),
            Event::Idle { .. }
            | Event::StoppedAtFloor { .. }
            | Event::FloorButtonPressed { .. }
            | Event::UpButtonPressed { .. }
            | Event::DownButtonPressed { .. } => None,
        })
        .collect();
    assert_eq!(passings, vec![(1, Direction::Up), (2, Direction::Up)]);
}

#[test]
fn go_to_floor_issued_while_draining_idle_takes_effect_in_the_same_iteration() {
    let challenge = Challenge::new(4, 1, vec![4], 0.0, Condition::Demo).unwrap();
    let mut world = World::new(&challenge, 0);

    // A new car says it is idle.
    assert_eq!(world.add_elevator(), Some(0));
    let drained = world.drain_events();
    assert_eq!(drained, vec![Event::Idle { elevator: 0 }]);

    // Responding to Idle mid-drain starts movement synchronously...
    world.go_to_floor(0, 2.0);
    let elevator = &world.elevators()[0];
    assert!(elevator.is_moving());
    assert_eq!(elevator.destination(), Some(2.0));

    // ...so the very next substep accelerates it (velocity integrates
    // into position one substep later - velocity-before-acceleration
    // Euler, research §3).
    world.step(DT_MAX);
    let elevator = &world.elevators()[0];
    assert!(elevator.velocity_y() != 0.0);
    world.step(DT_MAX);
    let elevator = &world.elevators()[0];
    assert!(elevator.y() < 150.0, "must have moved upward");
}

#[test]
fn stop_mid_flight_halts_between_floors_idle_with_no_arrival_and_no_dwell() {
    let mut world = quiet_world(4, 1);
    world.go_to_floor(0, 3.0);

    // Reach cruising speed, then stop.
    let mut t = 0.0;
    while world.elevators()[0].velocity_y().abs() < MAXSPEED {
        t += DT_MAX;
        substep(&mut world, t);
    }
    world.stop(0);
    assert_eq!(world.elevators()[0].destination(), None);

    // Run well past the halt: it must settle between floors, then idle.
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
    let post_stop_events: Vec<Event> = post_stop_events
        .into_iter()
        .map(|(_, event)| event)
        .collect();
    assert_eq!(
        post_stop_events,
        vec![Event::Idle { elevator: 0 }],
        "a stop() halt arrives nowhere and leaves the car idle"
    );
}

#[test]
fn a_new_trip_replaces_the_current_one() {
    let mut world = quiet_world(4, 1);

    world.go_to_floor(0, 2.0);
    world.go_to_floor(0, 2.0);
    assert_eq!(world.elevators()[0].destination(), Some(2.0));

    world.go_to_floor(0, 3.0);
    assert_eq!(world.elevators()[0].destination(), Some(3.0));

    let (events, _) = drive_until_arrival(&mut world, 6 * 60);
    assert!(events.iter().any(|(_, event)| *event
        == Event::StoppedAtFloor {
            elevator: 0,
            floor: 3
        }));
    assert!(
        !events.iter().any(|(_, event)| *event
            == Event::StoppedAtFloor {
                elevator: 0,
                floor: 2
            }),
        "the replaced trip's floor is passed, not stopped at"
    );
}

#[test]
fn idle_fires_one_second_after_the_last_arrival() {
    let mut world = quiet_world(4, 1);
    world.go_to_floor(0, 1.0);

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
    let after = t_idle.expect("a car with no trip must go idle after the dwell") - t_arrival;
    assert!((0.98..=1.06).contains(&after), "idle after {after:.4} s");
}

#[test]
fn stop_during_a_dwell_only_drops_the_trip() {
    let mut world = quiet_world(4, 1);
    world.go_to_floor(0, 1.0);
    let (_, samples) = drive_until_arrival(&mut world, 6 * 60);
    let (t_arrival, _, _) = *samples.last().unwrap();

    world.go_to_floor(0, 3.0);
    assert_eq!(world.elevators()[0].destination(), Some(3.0));

    world.stop(0);
    let elevator = &world.elevators()[0];
    assert_eq!(elevator.destination(), None);
    assert!(elevator.is_busy(), "stop() during a dwell keeps the dwell");

    // With no trip, the dwell ends in idleness, still on floor 1.
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
fn reopening_a_standing_car_causes_a_re_arrival() {
    let mut world = quiet_world(4, 1);

    // Standing at floor 0; the world's re-arrival rule does this when a
    // call button is pressed beside a parked elevator.
    world.reopen(0);
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
    let mut world = quiet_world(4, 1);

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

    world.go_to_floor(0, 2.0);
    drive_until_arrival(&mut world, 6 * 60);
    assert_eq!(world.elevators()[0].pressed_floors(), Vec::<usize>::new());
}
