// Research §5.1, the wiki's "easy solution": every elevator serves its
// own pressed buttons directly, idles back to floor 0, and every floor
// call is blindly given to elevator 0. No direction awareness, no load
// awareness — clears the first few challenges only.
//
// Rhai gotcha vs JS: a `for` loop shares ONE loop variable across
// iterations, so closures must capture a per-iteration shadow
// (`let elevator = elevator;`) or they would all see the last value.

fn init(elevators, floors) {
    for elevator in elevators {
        let elevator = elevator;
        elevator.on("floor_button_pressed", |floor_num| {
            elevator.go_to_floor(floor_num);
        });
        elevator.on("idle", || {
            elevator.go_to_floor(0);
        });
    }
    for floor in floors {
        let floor = floor;
        // The multi-event form binds both buttons to one zero-argument
        // handler, exactly like the original — the runtime drops the
        // unused event-name and floor arguments.
        floor.on("up_button_pressed down_button_pressed", || {
            elevators[0].go_to_floor(floor.floor_num());
        });
    }
}

fn update(dt, elevators, floors) {}
