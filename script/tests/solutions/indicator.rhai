// Research §5.2-style, "two sorted lists for up and down": global sorted
// request lists fed by the floor call buttons, idle cars claiming a
// whole list as one directional sweep, travel-order insertion into the
// destination queue (via set_destination_queue +
// check_destination_queue — the rhai idiom for the original's
// destinationQueue.splice), and indicator steering at every stop so only
// same-direction passengers board.

fn between(x, a, b) {
    (a <= x && x <= b) || (b <= x && x <= a)
}

fn init(elevators, floors) {
    let ups = [];
    let downs = [];

    for floor in floors {
        let floor = floor;
        floor.on("up_button_pressed", |f| {
            let level = f.floor_num;
            if !ups.contains(level) {
                ups.push(level);
                ups.sort();
            }
        });
        floor.on("down_button_pressed", |f| {
            let level = f.floor_num;
            if !downs.contains(level) {
                downs.push(level);
                downs.sort();
            }
        });
    }

    for elevator in elevators {
        let e = elevator;

        // Idle: claim the pending up calls as one ascending sweep, else
        // the down calls as one descending sweep, else park at the
        // lobby — with both indicators lit so anyone may board on
        // arrival. Overflow passengers re-press, refilling the lists
        // for the next idle car.
        e.on("idle", || {
            e.going_up_indicator = true;
            e.going_down_indicator = true;
            if ups.len() > 0 {
                while ups.len() > 0 {
                    e.go_to_floor(ups.remove(0));
                }
            } else if downs.len() > 0 {
                while downs.len() > 0 {
                    e.go_to_floor(downs.pop());
                }
            } else {
                e.go_to_floor(0);
            }
        });

        // New dropoffs are inserted into the queue in travel order, so
        // the car sweeps floors in one direction instead of ping-ponging
        // in press order.
        e.on("floor_button_pressed", |floor_num| {
            let queue = e.destination_queue;
            if queue.is_empty() {
                e.go_to_floor(floor_num);
            } else if !queue.contains(floor_num) {
                let placed = false;
                for i in 0..queue.len() {
                    if between(floor_num, e.current_floor, queue[i]) {
                        queue.insert(i, floor_num);
                        e.set_destination_queue(queue);
                        e.check_destination_queue();
                        placed = true;
                        break;
                    }
                }
                if !placed {
                    e.go_to_floor(floor_num);
                }
            }
        });

        // At each stop, indicate the direction of the next destination —
        // the boarding rule then keeps wrong-direction passengers out.
        e.on("stopped_at_floor", |floor_num| {
            let queue = e.destination_queue;
            if queue.is_empty() {
                e.going_up_indicator = true;
                e.going_down_indicator = true;
            } else {
                e.going_up_indicator = queue[0] > floor_num;
                e.going_down_indicator = queue[0] < floor_num;
            }
        });
    }
}
