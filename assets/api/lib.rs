//! The scripting surface. A program is Rust, run by rawr. It defines
//! `new`, whose return value is its state, and `update`, which
//! receives every event as a message and answers with commands.
//! Everything on these pages is in scope already - no `use` needed.

/// Runs once, at challenge start. Required. The returned value is the
/// program's state - any type you define - lent to `update` as
/// `&mut`, so changes there persist between messages.
///
///     struct Model {}
///
///     fn new() -> Model {
///         Model {}
///     }
fn new() -> Model;

/// Runs for every event. Required, with exactly this signature over
/// the type `new` returns. `message` says what happened; `elevators`
/// and `floors` are read-only snapshots of the world as it stands
/// (see `Elevator` and `Floor`). The returned commands are applied in
/// order, the moment `update` returns.
///
///     match message {
///         Message::Idle(elevator) => vec![go_to_floor(elevator, 0)],
///         Message::PassingFloor(e, _, Direction::Up) => vec![stop(e)],
///         _ => vec![],
///     }
fn update(
    model: &mut Model,
    message: Message,
    elevators: &[Elevator],
    floors: &[Floor],
) -> Vec<Command>;

/// Everything that can happen, one message per occurrence. Payloads
/// are elevator and floor indices, by position - index `elevators`
/// and `floors` with them directly. A `match` must cover every
/// variant; end with `_ => vec![]` to ignore the rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Message {
    /// `Tick(dt)`. Time: once per frame, before that frame's physics.
    /// `dt` is the frame's simulated seconds.
    Tick(f64),

    /// `Idle(elevator)`. The elevator's queue was checked while
    /// empty. Fires for every elevator at challenge start, and ~1 s
    /// after the last destination completes.
    Idle(usize),

    /// `FloorButtonPressed(elevator, floor)`. A rider pressed an unlit
    /// in-car destination button.
    FloorButtonPressed(usize, usize),

    /// `PassingFloor(elevator, floor, direction)`. About to pass
    /// `floor` without stopping, early enough that
    /// `go_to_floor_first(elevator, floor)` still makes the stop.
    PassingFloor(usize, usize, Direction),

    /// `StoppedAtFloor(elevator, floor)`. Arrived and snapped, fired
    /// before exit and boarding: indicator commands returned here
    /// affect who boards.
    StoppedAtFloor(usize, usize),

    /// `UpButtonPressed(floor)`. The floor's up call button went from
    /// unlit to lit. Re-fires when riders who could not board press
    /// again after an arrival cleared it.
    UpButtonPressed(usize),

    /// `DownButtonPressed(floor)`. Likewise, for down.
    DownButtonPressed(usize),
}

/// A direction of travel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
}

/// An instruction to the world, made by the functions below and
/// applied the moment `update` returns. Elevator indices out of the
/// bank are runtime errors; floors are clamped to the building.
pub enum Command { /* … */ }

/// Queues `floor` at the back of the elevator's queue. Deduplicated
/// against the adjacent queue entry only: 2, 2 is one stop; 2, 3, 2
/// visits 2 twice.
pub fn go_to_floor(elevator: usize, floor: usize) -> Command;

/// Queues `floor` at the front of the queue instead: go there first.
pub fn go_to_floor_first(elevator: usize, floor: usize) -> Command;

/// Clears the elevator's queue and halts it at the projected stopping
/// point - generally between floors, doors shut. Intended for
/// in-transit rescheduling; follow with `go_to_floor`.
pub fn stop(elevator: usize) -> Command;

/// Re-examines the queue now: non-empty starts toward the front entry;
/// empty (outside a dwell) fires `Message::Idle`. Queue edits made
/// through `set_destination_queue` apply on the next check.
pub fn check_destination_queue(elevator: usize) -> Command;

/// Replaces the queue, entries clamped to the building.
///
///     let mut queue = elevators[0].destination_queue.clone();
///     queue.insert(0, 3);
///     vec![set_destination_queue(0, queue), check_destination_queue(0)]
pub fn set_destination_queue(elevator: usize, queue: Vec<usize>) -> Command;

/// Sets the up lamp. Both lamps start on. A waiting rider boards only
/// if the lamp matching their direction is lit; the engine never
/// touches the lamps itself.
pub fn set_going_up_indicator(elevator: usize, on: bool) -> Command;

/// Sets the down lamp. See `set_going_up_indicator`.
pub fn set_going_down_indicator(elevator: usize, on: bool) -> Command;

// rawr checks the whole program before it runs, as rustc would: a
// type error, a missing match arm or a wrong `update` signature
// refuses to Apply, with rustc's error under the editor.
//
// A panic anywhere in your code - `panic!`, a failed `assert!`, an
// index out of bounds - pauses the run; the message surfaces under
// the editor with its line.
//
// The names on these pages are taken: define your own `Model`, helper
// functions and types, but not a second `Message` or `go_to_floor`.
//
// Runs are deterministic: same seed, same program, same result. The
// stats bar's Seed names the attempt.
