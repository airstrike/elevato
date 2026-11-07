// The elevato prelude: the game API, in Rust, appended to every player
// program before rawr compiles it. Player code names these items
// directly, the way a Rust program names its std prelude - no `use`
// needed. The `__elevato_*` functions at the bottom are the host's
// side of the boundary: rawr's typed handles carry only scalars,
// `String`s, `Option`s and exported structs, so the host builds every
// enum and every snapshot by calling these, and reads commands back
// through `__elevato_lower`. Players never call them.

/// A direction of travel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

/// One world occurrence, delivered to `update`. Payloads are plain
/// data - elevator and floor indices - never handles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Message {
    /// Time: once per frame, before that frame's physics, with the
    /// frame's simulated seconds.
    Tick(f64),
    /// The elevator's queue was checked while empty.
    Idle(usize),
    /// A rider pressed an unlit in-car destination button:
    /// `(elevator, floor)`.
    FloorButtonPressed(usize, usize),
    /// About to pass a floor without stopping:
    /// `(elevator, floor, direction)`.
    PassingFloor(usize, usize, Direction),
    /// Arrived and snapped, before exit and boarding:
    /// `(elevator, floor)`.
    StoppedAtFloor(usize, usize),
    /// The floor's up call button went from unlit to lit.
    UpButtonPressed(usize),
    /// The floor's down call button went from unlit to lit.
    DownButtonPressed(usize),
}

/// One car of the bank, as `update` sees it: a read-only snapshot,
/// rebuilt whenever the world changes.
#[derive(Debug, Clone, PartialEq)]
pub struct Elevator {
    pub current_floor: usize,
    pub max_passenger_count: usize,
    pub load_factor: f64,
    pub is_full: bool,
    pub destination_direction: Option<Direction>,
    pub destination_queue: Vec<usize>,
    pub pressed_floors: Vec<usize>,
    pub move_count: usize,
    pub is_busy: bool,
    pub is_moving: bool,
    pub is_on_a_floor: bool,
    pub going_up_indicator: bool,
    pub going_down_indicator: bool,
}

/// One floor of the building, as `update` sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct Floor {
    pub floor_num: usize,
    pub level: usize,
    pub up_pressed: bool,
    pub down_pressed: bool,
}

/// One instruction to the world, applied the moment `update` returns.
#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    GoToFloor {
        elevator: usize,
        floor: usize,
        force: bool,
    },
    Stop {
        elevator: usize,
    },
    CheckDestinationQueue {
        elevator: usize,
    },
    SetDestinationQueue {
        elevator: usize,
        queue: Vec<usize>,
    },
    SetGoingUpIndicator {
        elevator: usize,
        on: bool,
    },
    SetGoingDownIndicator {
        elevator: usize,
        on: bool,
    },
}

/// Queues `floor` at the back of the elevator's queue.
pub fn go_to_floor(elevator: usize, floor: usize) -> Command {
    Command::GoToFloor {
        elevator,
        floor,
        force: false,
    }
}

/// Queues `floor` at the front of the elevator's queue: go there first.
pub fn go_to_floor_first(elevator: usize, floor: usize) -> Command {
    Command::GoToFloor {
        elevator,
        floor,
        force: true,
    }
}

/// Clears the queue and halts at the projected stopping point.
pub fn stop(elevator: usize) -> Command {
    Command::Stop { elevator }
}

/// Re-examines the queue now.
pub fn check_destination_queue(elevator: usize) -> Command {
    Command::CheckDestinationQueue { elevator }
}

/// Replaces the queue.
pub fn set_destination_queue(elevator: usize, queue: Vec<usize>) -> Command {
    Command::SetDestinationQueue { elevator, queue }
}

/// Sets the up lamp.
pub fn set_going_up_indicator(elevator: usize, on: bool) -> Command {
    Command::SetGoingUpIndicator { elevator, on }
}

/// Sets the down lamp.
pub fn set_going_down_indicator(elevator: usize, on: bool) -> Command {
    Command::SetGoingDownIndicator { elevator, on }
}

fn __elevato_direction(direction: i64) -> Direction {
    if direction > 0 {
        Direction::Up
    } else {
        Direction::Down
    }
}

fn __elevato_message(kind: i64, first: usize, second: usize, direction: i64, dt: f64) -> Message {
    match kind {
        0 => Message::Tick(dt),
        1 => Message::Idle(first),
        2 => Message::FloorButtonPressed(first, second),
        3 => Message::PassingFloor(first, second, __elevato_direction(direction)),
        4 => Message::StoppedAtFloor(first, second),
        5 => Message::UpButtonPressed(first),
        _ => Message::DownButtonPressed(first),
    }
}

fn __elevato_elevator(
    current_floor: usize,
    max_passenger_count: usize,
    load_factor: f64,
    is_full: bool,
    direction: i64,
    destination_queue: Vec<usize>,
    pressed_floors: Vec<usize>,
    move_count: usize,
    is_busy: bool,
    is_moving: bool,
    is_on_a_floor: bool,
    going_up_indicator: bool,
    going_down_indicator: bool,
) -> Elevator {
    let destination_direction = if direction == 0 {
        None
    } else {
        Some(__elevato_direction(direction))
    };
    Elevator {
        current_floor,
        max_passenger_count,
        load_factor,
        is_full,
        destination_direction,
        destination_queue,
        pressed_floors,
        move_count,
        is_busy,
        is_moving,
        is_on_a_floor,
        going_up_indicator,
        going_down_indicator,
    }
}

fn __elevato_floor(floor_num: usize, up_pressed: bool, down_pressed: bool) -> Floor {
    Floor {
        floor_num,
        level: floor_num,
        up_pressed,
        down_pressed,
    }
}

fn __elevato_lower(commands: Vec<Command>) -> Vec<Vec<i64>> {
    let mut lowered = Vec::new();
    for command in commands {
        let words = match command {
            Command::GoToFloor {
                elevator,
                floor,
                force,
            } => vec![0, elevator as i64, floor as i64, force as i64],
            Command::Stop { elevator } => vec![1, elevator as i64],
            Command::CheckDestinationQueue { elevator } => vec![2, elevator as i64],
            Command::SetDestinationQueue { elevator, queue } => {
                let mut words = vec![3, elevator as i64];
                for floor in queue {
                    words.push(floor as i64);
                }
                words
            }
            Command::SetGoingUpIndicator { elevator, on } => vec![4, elevator as i64, on as i64],
            Command::SetGoingDownIndicator { elevator, on } => vec![5, elevator as i64, on as i64],
        };
        lowered.push(words);
    }
    lowered
}
