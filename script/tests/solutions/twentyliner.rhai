// Research §5.3-style ("Twentyliner"): no events at all — every update
// re-plans each elevator from scratch by polling the floors' call-button
// states. Down calls are encoded as negative floor numbers, exactly like
// the original (so a floor-0 up call, +0, has sign 0 and is only taken
// by direction-less cars — a faithful quirk). Pickups are considered
// only under half load and only along the car's current direction; the
// chosen floor is claimed from the global list, the car is stopped and
// re-dispatched, and the indicators are set to match its direction.

fn sgn(x) {
    if x > 0 { 1 } else if x < 0 { -1 } else { 0 }
}

fn init(elevators, floors) {}

fn update(dt, elevators, floors) {
    let calls = [];
    for f in floors {
        if f.up_pressed { calls.push(f.floor_num); }
        if f.down_pressed { calls.push(-f.floor_num); }
    }
    for e in elevators {
        let current = e.current_floor;
        let pressed = e.pressed_floors;

        // Direction: toward the furthest-up pressed floor that is not
        // the current one (the original's `.pop()` of the ascending
        // list); 0 when nothing is pressed.
        let dir = 0;
        for p in pressed {
            if p != current { dir = sgn(p - current); }
        }

        // Candidate pickups: only when lightly loaded; a directed car
        // takes only same-direction calls at-or-beyond its position.
        let options = [];
        if e.load_factor < 0.5 {
            for c in calls {
                let along = sgn(abs(c) - current);
                if dir == 0 || (sgn(c) == dir && (along == dir || along == 0)) {
                    options.push(c);
                }
            }
        }
        for p in pressed { options.push(p); }

        // Nearest useful floor wins (later entries win ties, like the
        // original's reduce).
        let found = false;
        let best = 0;
        for o in options {
            if !found || abs(abs(best) - current) >= abs(abs(o) - current) {
                best = o;
                found = true;
            }
        }

        if found {
            // Claim the call, drop any stale plan, indicate direction,
            // and go.
            calls = calls.filter(|c| c != best);
            e.stop();
            e.going_down_indicator = dir <= 0;
            e.going_up_indicator = dir >= 0;
            e.go_to_floor(abs(best));
        }
    }
}
