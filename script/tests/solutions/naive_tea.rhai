// The naive strategy, message dialect: every elevator serves its own
// buttons and idles to the ground floor; every call button dispatches
// elevator 0.

fn model() {
    #{}
}

fn update(message, elevators, floors) {
    switch message.kind {
        "idle" => message.elevator.go_to_floor(0),
        "floor_button_pressed" => message.elevator.go_to_floor(message.floor),
        "up_button_pressed" => elevators[0].go_to_floor(message.floor.floor_num),
        "down_button_pressed" => elevators[0].go_to_floor(message.floor.floor_num),
    }
}
