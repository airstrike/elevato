// Research §5.1, the wiki's "easy solution": every elevator serves its
// own pressed buttons directly, idles back to floor 0, and every floor
// call is blindly given to elevator 0. No direction awareness, no load
// awareness - clears the first few challenges only.

fn new() {
    #{}
}

fn update(message, elevators, floors) {
    switch message.kind {
        "idle" => go_to_floor(message.elevator, 0),
        "floor_button_pressed" => go_to_floor(message.elevator, message.floor),
        "up_button_pressed" | "down_button_pressed" => go_to_floor(0, message.floor)
    }
}
