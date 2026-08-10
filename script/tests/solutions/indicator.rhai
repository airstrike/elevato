// Research §5.2-style, "two sorted lists for up and down": global sorted
// request lists fed by the floor call buttons, idle cars claiming a
// whole list as one directional sweep, travel-order insertion into the
// destination queue (via set_destination_queue +
// check_destination_queue - the command idiom for the original's
// destinationQueue.splice), and indicator steering at every stop so only
// same-direction passengers board.

fn between(x, a, b) {
    (a <= x && x <= b) || (b <= x && x <= a)
}

fn new() {
    #{ ups: [], downs: [] }
}

fn update(message, elevators, floors) {
    switch message {
        Message::UpButtonPressed(floor) => {
            if !this.ups.contains(floor) {
                this.ups.push(floor);
                this.ups.sort();
            }
        }
        Message::DownButtonPressed(floor) => {
            if !this.downs.contains(floor) {
                this.downs.push(floor);
                this.downs.sort();
            }
        }
        // Idle: claim the pending up calls as one ascending sweep, else
        // the down calls as one descending sweep, else park at the
        // lobby - with both indicators lit so anyone may board on
        // arrival. Overflow passengers re-press, refilling the lists
        // for the next idle car.
        Message::Idle(e) => {
            let commands = [
                set_going_up_indicator(e, true),
                set_going_down_indicator(e, true)
            ];
            if this.ups.len() > 0 {
                while this.ups.len() > 0 {
                    commands.push(go_to_floor(e, this.ups.remove(0)));
                }
            } else if this.downs.len() > 0 {
                while this.downs.len() > 0 {
                    commands.push(go_to_floor(e, this.downs.pop()));
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
            let queue = elevators[e].destination_queue;
            if queue.is_empty() {
                go_to_floor(e, floor)
            } else if !queue.contains(floor) {
                let commands = [];
                let placed = false;
                for i in 0..queue.len() {
                    if between(floor, elevators[e].current_floor, queue[i]) {
                        queue.insert(i, floor);
                        commands.push(set_destination_queue(e, queue));
                        commands.push(check_destination_queue(e));
                        placed = true;
                        break;
                    }
                }
                if !placed {
                    commands.push(go_to_floor(e, floor));
                }
                commands
            }
        }
        // At each stop, indicate the direction of the next destination -
        // the boarding rule then keeps wrong-direction passengers out.
        Message::StoppedAtFloor(e, floor) => {
            let queue = elevators[e].destination_queue;
            if queue.is_empty() {
                [
                    set_going_up_indicator(e, true),
                    set_going_down_indicator(e, true)
                ]
            } else {
                [
                    set_going_up_indicator(e, queue[0] > floor),
                    set_going_down_indicator(e, queue[0] < floor)
                ]
            }
        }
    }
}
