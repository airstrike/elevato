//! The scripting API surface: the [`Message`] enum scripts match with
//! `switch`, the opaque [`Command`] type with its registered
//! constructor functions, the snapshot builders that turn world state
//! into the plain-data maps and arrays `update` receives, and engine
//! construction.
//!
//! Scripts never hold a reference into the world. State flows in as
//! snapshots rebuilt before every `update` call; effects flow out as
//! commands in `update`'s return value, applied by the runtime the
//! moment the call returns.

use elevato_core::World;
use elevato_core::event::Direction;
use rhai::{Array, Dynamic, Engine, EvalAltResult, Map, Position};

/// One world occurrence, delivered to `update` as an enum value the
/// script matches with the fork's qualified switch patterns:
/// `switch message { Message::Idle(elevator) => … }`. Payloads are
/// plain data - indices, numbers, `"up"`/`"down"` - never handles.
#[derive(Debug, Clone)]
pub(crate) enum Message {
    /// Time, once per frame, before that frame's physics.
    Tick { dt: f64 },
    /// The elevator's queue ran dry.
    Idle { elevator: usize },
    /// A button inside the cab was pressed.
    FloorButtonPressed { elevator: usize, floor: usize },
    /// About to pass a floor (fires only mid-flight, never for the
    /// destination itself).
    PassingFloor {
        elevator: usize,
        floor: usize,
        direction: Direction,
    },
    /// Arrived and doors open.
    StoppedAtFloor { elevator: usize, floor: usize },
    /// A waiting rider pressed the floor's up call button.
    UpButtonPressed { floor: usize },
    /// A waiting rider pressed the floor's down call button.
    DownButtonPressed { floor: usize },
}

/// One instruction to the world, minted by the constructor functions
/// registered on the engine (`go_to_floor(…)`, `stop(…)`, …) and
/// consumed by the runtime from `update`'s return value. Opaque to
/// scripts - a command cannot be inspected or altered, only returned.
///
/// Floor values are clamped to the building by core, exactly as the
/// original coerced them; elevator indices are *not* clamped - an index
/// the challenge does not have is a runtime error at application time.
#[derive(Debug, Clone)]
pub(crate) enum Command {
    /// `go_to_floor(elevator, floor)` / `(elevator, floor, force)`.
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
            Command::GoToFloor { .. } => "go_to_floor",
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

/// A position-less runtime error (rhai adds the call-site position when
/// it propagates through a script frame).
pub(crate) fn runtime_error(message: String) -> Box<EvalAltResult> {
    Box::new(EvalAltResult::ErrorRuntime(message.into(), Position::NONE))
}

/// A fresh engine with the elevato API registered. Compilation and the
/// runtime both build engines here, so the registered surface can never
/// drift between the two.
pub(crate) fn engine() -> Engine {
    let mut engine = Engine::new();
    // rhai's default parse-depth and call-stack limits differ between
    // debug and release builds (16 vs 32 expression levels in function
    // bodies, 8 vs 64 call levels) - tight enough that a realistic
    // program fails to parse in debug only. Pin generous values so a
    // program compiles and runs identically in both profiles. No
    // operations limit: per-update replanning strategies must never trip
    // a budget the original did not have.
    engine.set_max_expr_depths(128, 128);
    engine.set_max_call_levels(64);

    // The message enum: `switch` patterns over `Message::…` are
    // compile-time checked against this registration (unknown variants
    // and wrong binding arities refuse to parse).
    let up: Dynamic = "up".into();
    let down: Dynamic = "down".into();
    engine.register_enum::<Message>(
        "Message",
        &[
            ("Tick", 1),
            ("Idle", 1),
            ("FloorButtonPressed", 2),
            ("PassingFloor", 3),
            ("StoppedAtFloor", 2),
            ("UpButtonPressed", 1),
            ("DownButtonPressed", 1),
        ],
        move |message| match *message {
            Message::Tick { dt } => (0, vec![dt.into()]),
            Message::Idle { elevator } => (1, vec![(elevator as i64).into()]),
            Message::FloorButtonPressed { elevator, floor } => {
                (2, vec![(elevator as i64).into(), (floor as i64).into()])
            }
            Message::PassingFloor {
                elevator,
                floor,
                direction,
            } => (
                3,
                vec![
                    (elevator as i64).into(),
                    (floor as i64).into(),
                    match direction {
                        Direction::Up => up.clone(),
                        Direction::Down => down.clone(),
                    },
                ],
            ),
            Message::StoppedAtFloor { elevator, floor } => {
                (4, vec![(elevator as i64).into(), (floor as i64).into()])
            }
            Message::UpButtonPressed { floor } => (5, vec![(floor as i64).into()]),
            Message::DownButtonPressed { floor } => (6, vec![(floor as i64).into()]),
        },
    );

    // The command constructors. `go_to_floor` accepts int and float
    // floors, like the original's `Number()` coercion.
    engine
        .register_type_with_name::<Command>("Command")
        .register_fn("go_to_floor", |elevator: i64, floor: i64| {
            Command::GoToFloor {
                elevator,
                floor: floor as f64,
                force: false,
            }
        })
        .register_fn("go_to_floor", |elevator: i64, floor: f64| {
            Command::GoToFloor {
                elevator,
                floor,
                force: false,
            }
        })
        .register_fn("go_to_floor", |elevator: i64, floor: i64, force: bool| {
            Command::GoToFloor {
                elevator,
                floor: floor as f64,
                force,
            }
        })
        .register_fn("go_to_floor", |elevator: i64, floor: f64, force: bool| {
            Command::GoToFloor {
                elevator,
                floor,
                force,
            }
        })
        .register_fn("stop", |elevator: i64| Command::Stop { elevator })
        .register_fn("check_destination_queue", |elevator: i64| {
            Command::CheckDestinationQueue { elevator }
        })
        .register_fn(
            "set_destination_queue",
            |elevator: i64, queue: Array| -> Result<Command, Box<EvalAltResult>> {
                Ok(Command::SetDestinationQueue {
                    elevator,
                    queue: levels(queue)?,
                })
            },
        )
        .register_fn("set_going_up_indicator", |elevator: i64, on: bool| {
            Command::SetGoingUpIndicator { elevator, on }
        })
        .register_fn("set_going_down_indicator", |elevator: i64, on: bool| {
            Command::SetGoingDownIndicator { elevator, on }
        });
    engine
}

/// The `elevators` argument to `update`: one plain-data map per
/// elevator, rebuilt fresh before every call - state may have changed
/// since the previous message.
pub(crate) fn elevator_snapshots(world: &World) -> Array {
    world
        .elevators()
        .iter()
        .map(|elevator| {
            let mut map = Map::new();
            let mut put = |key: &str, value: Dynamic| {
                map.insert(key.into(), value);
            };
            put("current_floor", (elevator.current_floor() as i64).into());
            put("max_passenger_count", (elevator.capacity() as i64).into());
            put("load_factor", elevator.load_factor().into());
            put("is_full", elevator.is_full().into());
            put(
                "destination_direction",
                match elevator.destination_direction() {
                    Some(Direction::Up) => "up".into(),
                    Some(Direction::Down) => "down".into(),
                    None => "stopped".into(),
                },
            );
            // Integral levels come back as ints so queue entries compare
            // cleanly against floor numbers.
            let queue: Array = elevator
                .destination_queue()
                .iter()
                .map(|&level| {
                    if level.fract() == 0.0 {
                        Dynamic::from(level as i64)
                    } else {
                        Dynamic::from(level)
                    }
                })
                .collect();
            put("destination_queue", queue.into());
            let pressed: Array = elevator
                .pressed_floors()
                .into_iter()
                .map(|level| Dynamic::from(level as i64))
                .collect();
            put("pressed_floors", pressed.into());
            put("move_count", (elevator.move_count() as i64).into());
            put("is_busy", elevator.is_busy().into());
            put("is_moving", elevator.is_moving().into());
            put("is_on_a_floor", elevator.is_on_a_floor().into());
            put("going_up_indicator", elevator.going_up_indicator().into());
            put(
                "going_down_indicator",
                elevator.going_down_indicator().into(),
            );
            Dynamic::from(map)
        })
        .collect()
}

/// The `floors` argument to `update`: one plain-data map per floor,
/// rebuilt fresh before every call.
pub(crate) fn floor_snapshots(world: &World) -> Array {
    world
        .floors()
        .iter()
        .map(|floor| {
            let mut map = Map::new();
            let level = floor.level() as i64;
            map.insert("floor_num".into(), level.into());
            map.insert("level".into(), level.into());
            map.insert("up_pressed".into(), floor.up_pressed().into());
            map.insert("down_pressed".into(), floor.down_pressed().into());
            Dynamic::from(map)
        })
        .collect()
}

/// The numeric levels of a queue argument, or the original's
/// type-naming refusal.
fn levels(queue: Array) -> Result<Vec<f64>, Box<EvalAltResult>> {
    let mut levels = Vec::with_capacity(queue.len());
    for item in queue {
        let level = if let Ok(int) = item.as_int() {
            int as f64
        } else if let Ok(float) = item.as_float() {
            float
        } else {
            return Err(runtime_error(format!(
                "set_destination_queue expects numbers, found {}",
                item.type_name()
            )));
        };
        levels.push(level);
    }
    Ok(levels)
}
