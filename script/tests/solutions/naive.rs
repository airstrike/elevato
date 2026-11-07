// Research §5.1, the wiki's "easy solution": every elevator serves its
// own pressed buttons directly, idles back to floor 0, and every floor
// call is blindly given to elevator 0. No direction awareness, no load
// awareness - clears the first few challenges only.

struct Model {}

fn new() -> Model {
    Model {}
}

fn update(_model: &mut Model, message: Message, _elevators: &[Elevator], _floors: &[Floor]) -> Vec<Command> {
    match message {
        Message::Idle(elevator) => vec![go_to_floor(elevator, 0)],
        Message::FloorButtonPressed(elevator, floor) => vec![go_to_floor(elevator, floor)],
        Message::UpButtonPressed(floor) | Message::DownButtonPressed(floor) => {
            vec![go_to_floor(0, floor)]
        }
        Message::Tick(_) | Message::PassingFloor(..) | Message::StoppedAtFloor(..) => vec![],
    }
}
