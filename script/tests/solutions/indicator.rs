// Research §5.2-style, "two sorted lists for up and down": global sorted
// request lists fed by the floor call buttons, idle cars claiming a
// whole list as one directional sweep, travel-order insertion into the
// destination queue (via set_destination_queue +
// check_destination_queue - the command idiom for the original's
// destinationQueue.splice), and indicator steering at every stop so only
// same-direction passengers board.

struct Model {
    ups: Vec<usize>,
    downs: Vec<usize>,
}

fn between(x: usize, a: usize, b: usize) -> bool {
    (a <= x && x <= b) || (b <= x && x <= a)
}

fn new() -> Model {
    Model {
        ups: Vec::new(),
        downs: Vec::new(),
    }
}

fn update(model: &mut Model, message: Message, elevators: &[Elevator], _floors: &[Floor]) -> Vec<Command> {
    match message {
        Message::UpButtonPressed(floor) => {
            if !model.ups.contains(&floor) {
                model.ups.push(floor);
                model.ups.sort();
            }
            vec![]
        }
        Message::DownButtonPressed(floor) => {
            if !model.downs.contains(&floor) {
                model.downs.push(floor);
                model.downs.sort();
            }
            vec![]
        }
        // Idle: claim the pending up calls as one ascending sweep, else
        // the down calls as one descending sweep, else park at the
        // lobby - with both indicators lit so anyone may board on
        // arrival. Overflow passengers re-press, refilling the lists
        // for the next idle car.
        Message::Idle(e) => {
            let mut commands = vec![
                set_going_up_indicator(e, true),
                set_going_down_indicator(e, true),
            ];
            if !model.ups.is_empty() {
                for floor in std::mem::take(&mut model.ups) {
                    commands.push(go_to_floor(e, floor));
                }
            } else if !model.downs.is_empty() {
                while let Some(floor) = model.downs.pop() {
                    commands.push(go_to_floor(e, floor));
                }
            } else {
                commands.push(go_to_floor(e, 0));
            }
            commands
        }
        // New dropoffs are inserted into the queue in travel order, so
        // the car sweeps floors in one direction instead of ping-ponging
        // in press order.
        Message::FloorButtonPressed(e, floor) => {
            let mut queue = elevators[e].destination_queue.clone();
            let current = elevators[e].current_floor;
            if queue.is_empty() {
                vec![go_to_floor(e, floor)]
            } else if queue.contains(&floor) {
                vec![]
            } else {
                match queue.iter().position(|&target| between(floor, current, target)) {
                    Some(index) => {
                        queue.insert(index, floor);
                        vec![set_destination_queue(e, queue), check_destination_queue(e)]
                    }
                    None => vec![go_to_floor(e, floor)],
                }
            }
        }
        // At each stop, indicate the direction of the next destination -
        // the boarding rule then keeps wrong-direction passengers out.
        Message::StoppedAtFloor(e, floor) => match elevators[e].destination_queue.first() {
            None => vec![
                set_going_up_indicator(e, true),
                set_going_down_indicator(e, true),
            ],
            Some(&next) => vec![
                set_going_up_indicator(e, next > floor),
                set_going_down_indicator(e, next < floor),
            ],
        },
        Message::Tick(_) | Message::PassingFloor(..) => vec![],
    }
}
