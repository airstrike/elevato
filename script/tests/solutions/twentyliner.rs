// Research §5.3-style ("Twentyliner"): no events at all - every tick
// re-plans each elevator from scratch by polling the floors' call-button
// states. Down calls are encoded as negative floor numbers, exactly like
// the original (so a floor-0 up call, +0, has sign 0 and is only taken
// by direction-less cars - a faithful quirk). Pickups are considered
// only under half load and only along the car's current direction; the
// chosen floor is claimed from the global list, the car is stopped and
// re-dispatched, and the indicators are set to match its direction.

struct Model {}

fn new() -> Model {
    Model {}
}

fn update(_model: &mut Model, message: Message, elevators: &[Elevator], floors: &[Floor]) -> Vec<Command> {
    match message {
        Message::Tick(_) => plan(elevators, floors),
        _ => vec![],
    }
}

/// How far a (signed) call is from `current`.
fn distance(call: i64, current: i64) -> i64 {
    (call.abs() - current).abs()
}

fn plan(elevators: &[Elevator], floors: &[Floor]) -> Vec<Command> {
    let mut commands = Vec::new();
    let mut calls: Vec<i64> = Vec::new();
    for f in floors {
        if f.up_pressed {
            calls.push(f.floor_num as i64);
        }
        if f.down_pressed {
            calls.push(-(f.floor_num as i64));
        }
    }
    for (index, e) in elevators.iter().enumerate() {
        let current = e.current_floor as i64;

        // Direction: toward the furthest-up pressed floor that is not
        // the current one (the original's `.pop()` of the ascending
        // list); 0 when nothing is pressed.
        let mut dir = 0;
        for &p in &e.pressed_floors {
            if p as i64 != current {
                dir = (p as i64 - current).signum();
            }
        }

        // Candidate pickups: only when lightly loaded; a directed car
        // takes only same-direction calls at-or-beyond its position.
        let mut options: Vec<i64> = Vec::new();
        if e.load_factor < 0.5 {
            for &c in &calls {
                let along = (c.abs() - current).signum();
                if dir == 0 || (c.signum() == dir && (along == dir || along == 0)) {
                    options.push(c);
                }
            }
        }
        for &p in &e.pressed_floors {
            options.push(p as i64);
        }

        // Nearest useful floor wins (later entries win ties, like the
        // original's reduce).
        let mut best: Option<i64> = None;
        for &option in &options {
            let closer = match best {
                None => true,
                Some(best) => distance(best, current) >= distance(option, current),
            };
            if closer {
                best = Some(option);
            }
        }

        if let Some(best) = best {
            // Claim the call, drop any stale plan, indicate direction,
            // and go.
            calls.retain(|&c| c != best);
            commands.push(stop(index));
            commands.push(set_going_down_indicator(index, dir <= 0));
            commands.push(set_going_up_indicator(index, dir >= 0));
            commands.push(go_to_floor(index, best.abs() as usize));
        }
    }
    commands
}
