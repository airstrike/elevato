//! The host side of the scripting boundary: engine construction, the
//! wire shapes the prelude's `__elevato_*` bridge functions take and
//! return, and the host [`Command`] those bridges lower to.
//!
//! rawr's host boundary carries scalars, strings, options and exported
//! structs - not enums, not `Vec`s, not the player's own types. So the
//! game's enums and snapshot structs are Rust source in `prelude.rs`,
//! and the host builds them by calling the prelude's bridge functions
//! with plain integers, floats and booleans, holding the results as
//! opaque [`Dynamic`]s. Commands come back the same way: the prelude
//! lowers `Vec<Command>` to rows of integers that [`Command::decode`]
//! reads.

use elevato_core::World;
use elevato_core::elevator::Elevator;
use elevato_core::event::{Direction, Event};
use elevato_core::floor::Floor;
use rawr::{Array, Dynamic, Engine, EvalAltResult, Position};

/// The bridge that builds a `Message` from [`Message::arguments`].
pub(crate) const MESSAGE: &str = "__elevato_message";
/// The bridge that builds one `Elevator` snapshot.
pub(crate) const ELEVATOR: &str = "__elevato_elevator";
/// The bridge that builds one `Floor` snapshot.
pub(crate) const FLOOR: &str = "__elevato_floor";
/// The bridge that lowers `Vec<Command>` to integer rows.
pub(crate) const LOWER: &str = "__elevato_lower";

/// A fresh engine with elevato's limits. Compilation and the runtime
/// both build engines here, so the two can never drift.
pub(crate) fn engine() -> Engine {
    let mut engine = Engine::new();
    // rawr's default call-stack limit is 8 levels in debug builds and 64
    // in release - tight enough that a realistic program fails in debug
    // only. Pin the release value so a program runs identically in
    // both profiles. No operations limit: per-update replanning
    // strategies must never trip a budget the original did not have.
    engine.set_max_call_levels(64);
    engine
}

/// One world occurrence, as the host knows it before the prelude turns
/// it into the script's `Message`.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Message {
    /// Time, once per frame, before that frame's physics.
    Tick { dt: f64 },
    /// Anything the world raised.
    Event(Event),
}

impl Message {
    /// The `__elevato_message(kind, first, second, direction, dt)`
    /// arguments: `kind` is the variant's index in the prelude's
    /// `Message`, `direction` is 1 for up and -1 for down.
    pub(crate) fn arguments(self) -> Vec<Dynamic> {
        let (kind, first, second, direction, dt) = match self {
            Message::Tick { dt } => (0, 0, 0, 0, dt),
            Message::Event(Event::Idle { elevator }) => (1, elevator, 0, 0, 0.0),
            Message::Event(Event::FloorButtonPressed { elevator, floor }) => {
                (2, elevator, floor, 0, 0.0)
            }
            Message::Event(Event::PassingFloor {
                elevator,
                floor,
                direction,
            }) => (3, elevator, floor, direction_code(Some(direction)), 0.0),
            Message::Event(Event::StoppedAtFloor { elevator, floor }) => {
                (4, elevator, floor, 0, 0.0)
            }
            Message::Event(Event::UpButtonPressed { floor }) => (5, floor, 0, 0, 0.0),
            Message::Event(Event::DownButtonPressed { floor }) => (6, floor, 0, 0, 0.0),
        };
        vec![
            Dynamic::from(kind as i64),
            index(first),
            index(second),
            Dynamic::from(direction),
            Dynamic::from(dt),
        ]
    }
}

/// The `__elevato_elevator` arguments for one car, in the prelude
/// struct's field order.
pub(crate) fn elevator_arguments(elevator: &Elevator) -> Vec<Dynamic> {
    // Every queued level is integral: scripts can only queue `usize`
    // floors, and core clamps them to the building.
    let queue: Array = elevator
        .destination_queue()
        .iter()
        .map(|&level| Dynamic::from(level as i64))
        .collect();
    let pressed: Array = elevator.pressed_floors().into_iter().map(index).collect();
    vec![
        index(elevator.current_floor()),
        index(elevator.capacity()),
        Dynamic::from(elevator.load_factor()),
        Dynamic::from(elevator.is_full()),
        Dynamic::from(direction_code(elevator.destination_direction())),
        Dynamic::from(queue),
        Dynamic::from(pressed),
        index(elevator.move_count()),
        Dynamic::from(elevator.is_busy()),
        Dynamic::from(elevator.is_moving()),
        Dynamic::from(elevator.is_on_a_floor()),
        Dynamic::from(elevator.going_up_indicator()),
        Dynamic::from(elevator.going_down_indicator()),
    ]
}

/// The `__elevato_floor` arguments for one floor.
pub(crate) fn floor_arguments(floor: &Floor) -> Vec<Dynamic> {
    vec![
        index(floor.level()),
        Dynamic::from(floor.up_pressed()),
        Dynamic::from(floor.down_pressed()),
    ]
}

/// A `usize` as the script sees it: rawr represents every integer type
/// as an `i64` at run time.
fn index(value: usize) -> Dynamic {
    Dynamic::from(value as i64)
}

/// 1 for up, -1 for down, 0 for none, as the prelude decodes it.
fn direction_code(direction: Option<Direction>) -> i64 {
    match direction {
        Some(Direction::Up) => 1,
        Some(Direction::Down) => -1,
        None => 0,
    }
}

/// One instruction to the world, decoded from a row `__elevato_lower`
/// wrote and consumed by [`Command::apply`].
///
/// Floor values are clamped to the building by core, exactly as the
/// original coerced them; elevator indices are *not* clamped - an index
/// the challenge does not have is a runtime error at application time.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Command {
    /// `go_to_floor(elevator, floor)` / `go_to_floor_first(…)`.
    GoToFloor {
        elevator: i64,
        floor: f64,
        force: bool,
    },
    /// `stop(elevator)`.
    Stop { elevator: i64 },
    /// `check_destination_queue(elevator)`.
    CheckDestinationQueue { elevator: i64 },
    /// `set_destination_queue(elevator, queue)`.
    SetDestinationQueue { elevator: i64, queue: Vec<f64> },
    /// `set_going_up_indicator(elevator, on)`.
    SetGoingUpIndicator { elevator: i64, on: bool },
    /// `set_going_down_indicator(elevator, on)`.
    SetGoingDownIndicator { elevator: i64, on: bool },
}

impl Command {
    /// Decodes the rows `__elevato_lower` returned, in order.
    pub(crate) fn decode(lowered: Dynamic) -> Result<Vec<Self>, Box<EvalAltResult>> {
        let rows = lowered
            .into_array()
            .map_err(|found| malformed(format!("expected the lowered commands, found {found}")))?;
        rows.into_iter()
            .map(|row| {
                let words = row
                    .into_array()
                    .map_err(|found| malformed(format!("expected a command row, found {found}")))?
                    .into_iter()
                    .map(|word| {
                        word.as_int()
                            .map_err(|found| malformed(format!("expected an integer, found {found}")))
                    })
                    .collect::<Result<Vec<i64>, _>>()?;
                Self::from_words(&words)
            })
            .collect()
    }

    /// One row: the variant's index in the prelude's `Command`, the
    /// elevator, then the variant's own fields.
    fn from_words(words: &[i64]) -> Result<Self, Box<EvalAltResult>> {
        let command = match *words {
            [0, elevator, floor, force] => Command::GoToFloor {
                elevator,
                floor: floor as f64,
                force: force != 0,
            },
            [1, elevator] => Command::Stop { elevator },
            [2, elevator] => Command::CheckDestinationQueue { elevator },
            [3, elevator, ref queue @ ..] => Command::SetDestinationQueue {
                elevator,
                queue: queue.iter().map(|&level| level as f64).collect(),
            },
            [4, elevator, on] => Command::SetGoingUpIndicator {
                elevator,
                on: on != 0,
            },
            [5, elevator, on] => Command::SetGoingDownIndicator {
                elevator,
                on: on != 0,
            },
            _ => return Err(malformed(format!("unknown command row {words:?}"))),
        };
        Ok(command)
    }

    /// Applies the command to the world (`transformation-method`: a
    /// command is consumed by its application). Fails when the elevator
    /// index is outside the challenge's bank.
    pub(crate) fn apply(self, world: &mut World) -> Result<(), Box<EvalAltResult>> {
        let index = self.elevator_index(world)?;
        match self {
            Command::GoToFloor { floor, force, .. } => world.go_to_floor(index, floor, force),
            Command::Stop { .. } => world.stop(index),
            Command::CheckDestinationQueue { .. } => world.check_destination_queue(index),
            Command::SetDestinationQueue { queue, .. } => {
                world.elevator_mut(index).set_destination_queue(queue);
            }
            Command::SetGoingUpIndicator { on, .. } => {
                world.elevator_mut(index).set_going_up_indicator(on);
            }
            Command::SetGoingDownIndicator { on, .. } => {
                world.elevator_mut(index).set_going_down_indicator(on);
            }
        }
        Ok(())
    }

    /// The constructor name, for error messages.
    fn name(&self) -> &'static str {
        match self {
            Command::GoToFloor { force: false, .. } => "go_to_floor",
            Command::GoToFloor { force: true, .. } => "go_to_floor_first",
            Command::Stop { .. } => "stop",
            Command::CheckDestinationQueue { .. } => "check_destination_queue",
            Command::SetDestinationQueue { .. } => "set_destination_queue",
            Command::SetGoingUpIndicator { .. } => "set_going_up_indicator",
            Command::SetGoingDownIndicator { .. } => "set_going_down_indicator",
        }
    }

    /// The raw elevator argument the script passed.
    fn elevator(&self) -> i64 {
        match *self {
            Command::GoToFloor { elevator, .. }
            | Command::Stop { elevator }
            | Command::CheckDestinationQueue { elevator }
            | Command::SetDestinationQueue { elevator, .. }
            | Command::SetGoingUpIndicator { elevator, .. }
            | Command::SetGoingDownIndicator { elevator, .. } => elevator,
        }
    }

    /// The validated elevator index, or a clear error naming the
    /// command. Floors are clamped by core; elevators never are.
    fn elevator_index(&self, world: &World) -> Result<usize, Box<EvalAltResult>> {
        let count = world.elevators().len();
        usize::try_from(self.elevator())
            .ok()
            .filter(|&index| index < count)
            .ok_or_else(|| {
                runtime_error(format!(
                    "{}: no elevator {} - this challenge has {count} (indices 0 to {})",
                    self.name(),
                    self.elevator(),
                    count - 1
                ))
            })
    }
}

/// A position-less runtime error: a command is applied after `update`
/// returns, so there is no script position to name.
pub(crate) fn runtime_error(message: String) -> Box<EvalAltResult> {
    Box::new(EvalAltResult::ErrorRuntime(message.into(), Position::NONE))
}

/// A bridge returned something the prelude cannot produce - a defect
/// in elevato, never in the player's program.
fn malformed(detail: String) -> Box<EvalAltResult> {
    runtime_error(format!("elevato's command bridge is broken: {detail}"))
}
