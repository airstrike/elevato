// The default starter program: whenever an elevator goes idle, send it
// to floor 0 and then floor 1.

struct Model {} // Our state. We don't need any yet.

fn new() -> Model {
    Model {}
}

fn update(model: &mut Model, message: Message, elevators: &[Elevator], floors: &[Floor]) -> Vec<Command> {
    match message {
        // Whenever an elevator is idle (has no more queued
        // destinations) ...
        Message::Idle(elevator) => vec![
            // let's go to all the floors (or did we forget one?)
            go_to_floor(elevator, 0),
            go_to_floor(elevator, 1),
        ],
        // Everything else, we ignore for now.
        _ => vec![],
    }
}
