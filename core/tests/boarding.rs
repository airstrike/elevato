//! Passenger-lifecycle integration tests: direction-filtered boarding,
//! overflow re-pressing and the re-arrival scan, the 1 s board-walk
//! before the destination button press, and the walk-off feeding
//! `max_wait_time` until removal. All runs go through the headless
//! runner - it is the only driver.

use elevato_core::challenge::{Challenge, Condition};
use elevato_core::controller::Controller;
use elevato_core::event::Event;
use elevato_core::{World, headless};

/// Sweeps its single elevator 0 → 1 → 2 → 3 → 0 forever, forcing the
/// down indicator off at every stop (during `StoppedAtFloor` dispatch -
/// before that same arrival's boarding runs), and audits at every event
/// that no down-going passenger is ever aboard.
struct UpSweeper {
    violations: usize,
    saw_waiting_down_goer: bool,
}

impl Controller for UpSweeper {
    fn init(&mut self, _world: &mut World) {}

    fn on_event(&mut self, world: &mut World, event: Event) {
        match event {
            Event::StoppedAtFloor { elevator, .. } => {
                world.elevator_mut(elevator).set_going_up_indicator(true);
                world.elevator_mut(elevator).set_going_down_indicator(false);
            }
            Event::Idle { elevator } => {
                let floors = world.floors().len();
                let next = (world.elevators()[elevator].current_floor() + 1) % floors;
                world.go_to_floor(elevator, next as f64, false);
            }
            Event::PassingFloor { .. }
            | Event::FloorButtonPressed { .. }
            | Event::UpButtonPressed { .. }
            | Event::DownButtonPressed { .. } => {}
        }
        // A passenger's current_floor stays the boarding floor while
        // aboard, so "aboard and bound lower" means a down-goer boarded.
        for passenger in world.passengers() {
            let down_goer = passenger.destination_floor() < passenger.current_floor();
            if passenger.aboard().is_some() && down_goer {
                self.violations += 1;
            }
            if passenger.is_waiting() && down_goer {
                self.saw_waiting_down_goer = true;
            }
        }
    }
}

#[test]
fn direction_filtered_boarding_keeps_down_goers_off_an_up_sweep() {
    let challenge = Challenge::new(4, 1, vec![4], 1.0, Condition::Demo).unwrap();
    let mut world = World::new(&challenge, 7);
    let mut sweeper = UpSweeper {
        violations: 0,
        saw_waiting_down_goer: false,
    };
    let report = headless::run(&mut world, &mut sweeper, 3600, 1);

    assert_eq!(sweeper.violations, 0, "a down-going passenger boarded");
    assert!(
        sweeper.saw_waiting_down_goer,
        "the seed produced no down-going passengers to filter"
    );
    assert!(
        report.stats.transported() > 0,
        "up-going passengers must still board and be delivered"
    );
    assert!(
        world
            .passengers()
            .iter()
            .any(|passenger| passenger.is_waiting()
                && passenger.destination_floor() < passenger.current_floor()),
        "down-goers should still be stranded when the run ends"
    );
}

/// Stages of the overflow choreography, in order.
enum Stage {
    /// Both elevators blind (indicators off) heading for the top; floor-0
    /// waiters accumulate under a lit, unclearable up button.
    Accumulate,
    /// The four-slot elevator descends blind and parks at floor 0 without
    /// clearing the button or boarding anyone.
    Park,
    /// Up indicators restored; the one-slot elevator descends to trigger
    /// the overflow.
    Descend,
    /// Watching for the re-press and the re-dispatched standing elevator.
    Watch,
}

struct Choreographer {
    stage: Stage,
    /// Spawn-order index of the passenger expected to overflow the
    /// one-slot elevator.
    overflow: Option<usize>,
    repress_seen: bool,
    rearrival_seen: bool,
}

impl Controller for Choreographer {
    fn init(&mut self, world: &mut World) {
        let top = (world.floors().len() - 1) as f64;
        for elevator in 0..world.elevators().len() {
            world.elevator_mut(elevator).set_going_up_indicator(false);
            world.elevator_mut(elevator).set_going_down_indicator(false);
            world.go_to_floor(elevator, top, false);
        }
    }

    fn update(&mut self, world: &mut World, _dt: f64) {
        match self.stage {
            Stage::Accumulate => {
                let waiting = world
                    .passengers()
                    .iter()
                    .filter(|passenger| passenger.is_waiting() && passenger.current_floor() == 0)
                    .count();
                if waiting >= 2 {
                    world.go_to_floor(1, 0.0, false);
                    self.stage = Stage::Park;
                }
            }
            Stage::Park => {
                let big = &world.elevators()[1];
                let parked = big.current_floor() == 0
                    && big.is_on_a_floor()
                    && !big.is_moving()
                    && !big.is_busy()
                    && big.destination_queue().is_empty();
                if parked {
                    world.elevator_mut(0).set_going_up_indicator(true);
                    world.elevator_mut(1).set_going_up_indicator(true);
                    world.go_to_floor(0, 0.0, false);
                    self.stage = Stage::Descend;
                }
            }
            Stage::Descend | Stage::Watch => {}
        }
    }

    fn on_event(&mut self, world: &mut World, event: Event) {
        match self.stage {
            Stage::Descend => {
                if event
                    == (Event::StoppedAtFloor {
                        elevator: 0,
                        floor: 0,
                    })
                {
                    // Boarding runs right after this dispatch: the first
                    // waiter (spawn order) takes the single slot, the
                    // second overflows.
                    let waiting: Vec<usize> = world
                        .passengers()
                        .iter()
                        .enumerate()
                        .filter(|(_, passenger)| {
                            passenger.is_waiting() && passenger.current_floor() == 0
                        })
                        .map(|(index, _)| index)
                        .collect();
                    assert!(waiting.len() >= 2, "choreography lost its waiters");
                    self.overflow = Some(waiting[1]);
                    self.stage = Stage::Watch;
                }
            }
            Stage::Watch => {
                if event == (Event::UpButtonPressed { floor: 0 }) {
                    self.repress_seen = true;
                }
                if event
                    == (Event::StoppedAtFloor {
                        elevator: 1,
                        floor: 0,
                    })
                {
                    self.rearrival_seen = true;
                }
            }
            Stage::Accumulate | Stage::Park => {}
        }
    }
}

#[test]
fn a_full_elevator_leaves_the_overflow_passenger_to_re_press_and_a_standing_elevator_collects_them()
{
    let challenge = Challenge::new(4, 2, vec![1, 4], 2.0, Condition::Demo).unwrap();
    let mut world = World::new(&challenge, 3);
    let mut choreographer = Choreographer {
        stage: Stage::Accumulate,
        overflow: None,
        repress_seen: false,
        rearrival_seen: false,
    };
    headless::run(&mut world, &mut choreographer, 1800, 1);

    assert!(
        matches!(choreographer.stage, Stage::Watch),
        "the choreography never reached the overflow"
    );
    assert!(
        choreographer.repress_seen,
        "the overflow passenger must re-press the cleared call button"
    );
    assert!(
        choreographer.rearrival_seen,
        "the re-arrival scan must re-dispatch the standing elevator"
    );
    let aboard_small = world
        .passengers()
        .iter()
        .filter(|passenger| passenger.aboard() == Some(0))
        .count();
    assert_eq!(aboard_small, 1, "the one-slot elevator boards exactly one");
    let overflow = choreographer.overflow.unwrap();
    assert_eq!(
        world.passengers()[overflow].aboard(),
        Some(1),
        "the overflow passenger must board the re-dispatched elevator"
    );
}

/// Records when the first arrival and the first in-elevator button press
/// happen; commands nothing (floor-0 spawns board the parked elevator
/// through the re-arrival rule on their own).
struct Recorder {
    first_stop: Option<f64>,
    first_press: Option<f64>,
}

impl Controller for Recorder {
    fn init(&mut self, _world: &mut World) {}

    fn on_event(&mut self, world: &mut World, event: Event) {
        match event {
            Event::StoppedAtFloor { .. } => {
                if self.first_stop.is_none() {
                    self.first_stop = Some(world.elapsed());
                }
            }
            Event::FloorButtonPressed { .. } => {
                if self.first_press.is_none() {
                    self.first_press = Some(world.elapsed());
                }
            }
            Event::Idle { .. }
            | Event::PassingFloor { .. }
            | Event::UpButtonPressed { .. }
            | Event::DownButtonPressed { .. } => {}
        }
    }
}

#[test]
fn the_destination_button_press_lands_one_second_after_boarding_not_at_boarding() {
    let challenge = Challenge::new(4, 1, vec![4], 0.5, Condition::Demo).unwrap();
    let mut world = World::new(&challenge, 2);
    let mut recorder = Recorder {
        first_stop: None,
        first_press: None,
    };
    headless::run(&mut world, &mut recorder, 1800, 1);

    let stop = recorder
        .first_stop
        .expect("a floor-0 spawn must trigger a boarding re-arrival");
    let press = recorder
        .first_press
        .expect("the boarder must press its destination button");
    let delay = press - stop;
    assert!(
        (0.98..=1.10).contains(&delay),
        "destination button pressed {delay:.4} s after boarding, expected ≈ 1.0 s"
    );
}

/// A naive single-elevator ferry that also samples
/// `(elapsed, max_wait_time, transported, present)` once per frame.
struct Ferry {
    history: Vec<(f64, f64, usize, usize)>,
}

impl Controller for Ferry {
    fn init(&mut self, _world: &mut World) {}

    fn update(&mut self, world: &mut World, _dt: f64) {
        self.history.push((
            world.elapsed(),
            world.stats().max_wait_time(),
            world.stats().transported(),
            world.passengers().len(),
        ));
    }

    fn on_event(&mut self, world: &mut World, event: Event) {
        match event {
            Event::FloorButtonPressed { elevator, floor } => {
                world.go_to_floor(elevator, floor as f64, false)
            }
            Event::UpButtonPressed { floor } | Event::DownButtonPressed { floor } => {
                world.go_to_floor(0, floor as f64, false)
            }
            Event::Idle { .. } | Event::PassingFloor { .. } | Event::StoppedAtFloor { .. } => {}
        }
    }
}

#[test]
fn max_wait_time_keeps_climbing_through_the_walk_off_and_plateaus_at_removal() {
    // Spawn interval 10 s: the first passenger is delivered, walks off,
    // and is removed while nobody else exists - so every change (and the
    // plateau) is attributable to the walk-off alone.
    let challenge = Challenge::new(3, 1, vec![4], 0.1, Condition::Demo).unwrap();
    let mut world = World::new(&challenge, 5);
    let mut ferry = Ferry {
        history: Vec::new(),
    };
    headless::run(&mut world, &mut ferry, 570, 1);

    let exit = ferry
        .history
        .iter()
        .position(|&(_, _, transported, _)| transported == 1)
        .expect("the first passenger must be delivered");
    let (t_exit, max_at_exit, _, present) = ferry.history[exit];
    assert!(
        t_exit < 6.0,
        "delivery at {t_exit:.2} s breaks the isolation window"
    );
    assert_eq!(
        present, 1,
        "the walk-off passenger must be the only one present"
    );

    // ~0.5 s into the walk-off (every walk-off lasts at least 1 s): still
    // present, and the maximum has climbed by about that much.
    let mid = ferry
        .history
        .iter()
        .find(|&&(t, ..)| t >= t_exit + 0.5)
        .unwrap();
    assert_eq!(mid.3, 1, "must still be walking off 0.5 s after exiting");
    assert!(
        mid.1 > max_at_exit + 0.45,
        "max wait must keep climbing during the walk-off ({} vs {max_at_exit})",
        mid.1
    );

    // Walk-offs cap at 1.5 s: two seconds after the exit the world is
    // empty and the maximum plateaus exactly.
    let after = ferry
        .history
        .iter()
        .find(|&&(t, ..)| t >= t_exit + 2.0)
        .unwrap();
    let later = ferry
        .history
        .iter()
        .find(|&&(t, ..)| t >= t_exit + 3.0)
        .unwrap();
    assert_eq!(
        after.3, 0,
        "the walker must be removed by 2 s after exiting"
    );
    assert_eq!(
        after.1, later.1,
        "max wait must plateau once nobody is present"
    );
}
