//! The scripting surface. A program defines `new`, whose return value
//! is its state, and `update`, which receives every event as a message
//! and answers with commands.

/// Runs once, at challenge start. Required. The returned value is the
/// program's state, bound as `this` inside `update` - mutations there
/// persist between messages. `#{}` is a fine start.
fn new() -> Model;

/// Runs for every event. Required. `message` is a `Message`; match it
/// with `switch`, binding payloads by position. `elevators` and
/// `floors` are arrays of read-only snapshot data (see `Elevator` and
/// `Floor`), rebuilt before every call. Return a `Command`, an array
/// of them (applied in order), or nothing.
///
///     switch message {
///         Message::Idle(elevator) => go_to_floor(elevator, 0),
///         Message::PassingFloor(e, floor, dir) if dir == "up" => stop(e),
///     }
fn update(message, elevators, floors) -> Command;

/// Everything that can happen, one message per occurrence. Payloads
/// bind by position; `_` discards one. An unmatched message falls out
/// of the `switch` as `()` and is ignored - no default arm needed.
/// Unknown variants and wrong arities refuse to compile.
pub enum Message {
    /// Time: once per frame, before that frame's physics. `dt` is the
    /// frame's simulated seconds.
    Tick(dt: f64),

    /// The elevator's queue was checked while empty. Fires for every
    /// elevator at challenge start, and ~1 s after the last
    /// destination completes.
    Idle(elevator: i64),

    /// A rider pressed an unlit in-car destination button.
    FloorButtonPressed(elevator: i64, floor: i64),

    /// About to pass `floor` without stopping, early enough that
    /// `go_to_floor(elevator, floor, true)` still makes the stop.
    /// `direction` is "up" or "down".
    PassingFloor(elevator: i64, floor: i64, direction: String),

    /// Arrived and snapped, fired before exit and boarding: indicator
    /// commands returned here affect who boards.
    StoppedAtFloor(elevator: i64, floor: i64),

    /// The floor's up call button went from unlit to lit. Re-fires
    /// when riders who could not board press again after an arrival
    /// cleared it.
    UpButtonPressed(floor: i64),

    /// Likewise, for down.
    DownButtonPressed(floor: i64),
}

/// An instruction to the world, minted by the constructors below and
/// applied the moment `update` returns. Elevator indices out of the
/// bank are runtime errors; floor values are clamped to the building.
pub struct Command;

/// Queues floor `floor` for elevator `elevator`, clamped to the
/// building. Deduplicated against the adjacent queue entry only: 2, 2
/// is one stop; 2, 3, 2 visits 2 twice. Accepts ints and floats.
pub fn go_to_floor(elevator: i64, floor: i64) -> Command;

/// With `force`, the floor goes to the front of the queue instead of
/// the back.
pub fn go_to_floor(elevator: i64, floor: i64, force: bool) -> Command;

/// Clears the elevator's queue and halts it at the projected stopping
/// point - generally between floors, doors shut. Intended for
/// in-transit rescheduling; follow with `go_to_floor`.
pub fn stop(elevator: i64) -> Command;

/// Re-examines the queue now: non-empty starts toward the front entry;
/// empty (outside a dwell) fires `Message::Idle`. Queue edits made
/// through `set_destination_queue` apply on the next check.
pub fn check_destination_queue(elevator: i64) -> Command;

/// Replaces the queue, entries clamped to the building.
///
///     let queue = elevators[0].destination_queue;
///     queue.insert(0, 3);
///     [set_destination_queue(0, queue), check_destination_queue(0)]
pub fn set_destination_queue(elevator: i64, queue: Vec<f64>) -> Command;

/// Sets the up lamp. Both lamps start on. A waiting rider boards only
/// if the lamp matching their direction is lit; the engine never
/// touches the lamps itself.
pub fn set_going_up_indicator(elevator: i64, on: bool) -> Command;

/// Sets the down lamp. See `set_going_up_indicator`.
pub fn set_going_down_indicator(elevator: i64, on: bool) -> Command;

// Exceptions thrown anywhere in user code pause the run; the message
// surfaces under the editor. Returning anything that is not a command
// (or an array of commands, or nothing) is an error too.
//
// Top-level statements run once, before `new`, and functions cannot
// see top-level variables - cross-message state lives in the model
// (`this`).
//
// Runs are deterministic: same seed, same program, same result. The
// stats bar's Seed names the attempt.
