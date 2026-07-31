// The original's default starter program: whenever elevator 0 goes idle,
// send it to floor 0 and then floor 1.

fn init(elevators, floors) {
    let elevator = elevators[0]; // Let's use the first elevator

    // Whenever the elevator is idle (has no more queued destinations) ...
    elevator.on("idle", || {
        // let's go to all the floors (or did we forget one?)
        elevator.go_to_floor(0);
        elevator.go_to_floor(1);
    });
}

fn update(dt, elevators, floors) {
    // We normally don't need to do anything here
}
