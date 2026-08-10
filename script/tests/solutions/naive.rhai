// Research §5.1, the wiki's "easy solution": every elevator serves its
// own pressed buttons directly, idles back to floor 0, and every floor
// call is blindly given to elevator 0. No direction awareness, no load
// awareness - clears the first few challenges only.

fn new() {
    #{}
}

fn update(message, elevators, floors) {
    switch message {
        Message::Idle(elevator) => go_to_floor(elevator, 0),
        Message::FloorButtonPressed(elevator, floor) => go_to_floor(elevator, floor),
        Message::UpButtonPressed(floor) => go_to_floor(0, floor),
        Message::DownButtonPressed(floor) => go_to_floor(0, floor)
    }
}
