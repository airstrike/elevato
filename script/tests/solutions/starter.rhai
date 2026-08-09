// The default starter program: whenever an elevator goes idle, send it
// to floor 0 and then floor 1.

fn new() {
    #{} // Our state. We don't need any yet.
}

fn update(message, elevators, floors) {
    switch message.kind {
        // Whenever an elevator is idle (has no more queued
        // destinations) ...
        "idle" => [
            // let's go to all the floors (or did we forget one?)
            go_to_floor(message.elevator, 0),
            go_to_floor(message.elevator, 1)
        ]
    }
}
