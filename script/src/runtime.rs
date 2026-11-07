//! The frame driver: owns the engine, the compiled AST, the model, and
//! the world - directly, by value - and replays the core driver
//! contract ([`elevato_core::world`] module docs) one frame at a time.
//!
//! This deliberately duplicates the ~30-line substep/dispatch loop of
//! [`elevato_core::headless::run`] - the runtime cannot implement
//! [`elevato_core::controller::Controller`] because the trait's hooks
//! are infallible while a script error must abort the frame and pause
//! playback (see `.claude/DECISIONS.md` D6/D9). The duplication is
//! pinned by the parity test in `tests/solutions.rs`: the scripted
//! naive solution must produce byte-identical final stats to a native
//! controller run through `core::headless`.
//!
//! Scripts never touch the world. Per message the runtime hands
//! `update` the model as its `&mut` first argument, the message, and
//! snapshots of the world, then applies the returned commands
//! immediately - before the next message dispatches - so a command
//! cascade unfolds in exactly the order the original's synchronous
//! callbacks did. Snapshots are rebuilt whenever the world may have
//! changed (a physics step, the arrival stage, an applied command), so
//! every message sees the world as it stands.

use elevato_core::World;
use elevato_core::challenge::{Challenge, Outcome};
use elevato_core::stats::Stats;
use elevato_core::world::DT_MAX;
use rawr::{AST, Array, CallFnOptions, Dynamic, Engine, EvalAltResult, Scope};

use crate::api::{self, Command, Message};
use crate::{Error, Program};

/// Hard cap on drain rounds per dispatch point, mirroring
/// [`elevato_core::headless`]: a command cascade past this many rounds
/// has its remaining events deferred to the next dispatch point.
const DISPATCH_ROUNDS: usize = 128;

/// A running scripted simulation, minted from a [`Program`] plus a
/// challenge and seed (`transformation-method`): construction builds the
/// world, boots the model via `fn new`, and fires the initial idle
/// round, so a `Runtime` that exists is already under way -
/// [`Runtime::frame`] does the rest.
pub struct Runtime {
    engine: Engine,
    ast: AST,
    world: World,
    /// The value `fn new` returned, lent to every `update` call as its
    /// `&mut` model; mutations persist.
    model: Dynamic,
    /// The `elevators` and `floors` slices as the script holds them,
    /// built from the world as it stood after the last change; `None`
    /// once the world has moved on.
    snapshots: Option<Snapshots>,
}

/// The world as `update` sees it, built by the prelude's bridges.
struct Snapshots {
    elevators: Dynamic,
    floors: Dynamic,
}

impl std::fmt::Debug for Runtime {
    // Hand-written because `rawr::Engine` has no `Debug`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("world", &self.world)
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

impl Runtime {
    /// Builds the world for `challenge`, evaluates the program's
    /// top-level statements, boots the model via `fn new()`, and fires
    /// the initial idle round exactly like the original's `world.init()`
    /// (and [`elevato_core::headless::run`]).
    pub fn new(program: Program, challenge: &Challenge, seed: u64) -> Result<Self, Error> {
        let Program { ast } = program;
        let mut runtime = Self {
            engine: api::engine(),
            ast,
            world: World::new(challenge, seed),
            model: Dynamic::UNIT,
            snapshots: None,
        };

        // Top-level statements run exactly once, here, before the boot
        // function - every later call skips them.
        let options = CallFnOptions::new().eval_ast(true);
        runtime.model = runtime.engine.call_fn_with_options(
            options,
            &mut Scope::new(),
            &runtime.ast,
            "new",
            (),
        )?;

        for index in 0..runtime.world.elevators().len() {
            runtime.world.check_destination_queue(index);
        }
        runtime.dispatch()?;
        Ok(runtime)
    }

    /// Advances one frame of `substeps` × [`DT_MAX`] simulated seconds:
    /// a `Tick` message with the whole frame dt first, then per substep -
    /// step physics, dispatch drained events, process staged arrivals,
    /// dispatch again - breaking the moment the challenge decides. A
    /// returned error aborts the frame; the caller pauses on it.
    pub fn frame(&mut self, substeps: usize) -> Result<(), Error> {
        if self.world.ended() {
            return Ok(());
        }
        // Time is a message like any other.
        let dt = substeps as f64 * DT_MAX;
        self.deliver(Message::Tick { dt })?;

        for _ in 0..substeps {
            if self.world.ended() {
                break;
            }
            self.world.step(DT_MAX);
            self.snapshots = None;
            self.dispatch()?;
            self.world.process_arrivals();
            self.snapshots = None;
            self.dispatch()?;
        }
        Ok(())
    }

    /// The live statistics snapshot.
    pub fn stats(&self) -> Stats {
        *self.world.stats()
    }

    /// How the challenge stands.
    pub fn outcome(&self) -> Outcome {
        self.world.outcome()
    }

    /// Whether the challenge has decided.
    pub fn ended(&self) -> bool {
        self.world.ended()
    }

    /// Read access to the world, for rendering.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Drains the event queue into `update` calls until quiescent
    /// (capped at [`DISPATCH_ROUNDS`]).
    fn dispatch(&mut self) -> Result<(), Error> {
        for _ in 0..DISPATCH_ROUNDS {
            let events = self.world.drain_events();
            if events.is_empty() {
                return Ok(());
            }
            for event in events {
                self.deliver(Message::Event(event))?;
            }
        }
        Ok(())
    }

    /// Folds one message into the model and applies the returned
    /// commands: `update` runs with the model lent as its `&mut` first
    /// argument (mutations persist), and the commands are applied before
    /// this call returns - i.e. before the next message dispatches.
    fn deliver(&mut self, message: Message) -> Result<(), Error> {
        let message = self.call(api::MESSAGE, message.arguments())?;
        let snapshots = match self.snapshots.take() {
            Some(snapshots) => snapshots,
            None => self.snapshot()?,
        };
        // Arguments are consumed by the call, so each one gets its own
        // copy; `update` only borrows them.
        let (elevators, floors) = (snapshots.elevators.clone(), snapshots.floors.clone());
        self.snapshots = Some(snapshots);

        // rawr passes a function's leading `&mut` parameter as the
        // bound `this`, so the model goes in by pointer and the rest by
        // value.
        let options = CallFnOptions::new()
            .eval_ast(false)
            .bind_this_ptr(&mut self.model);
        let commands: Dynamic = self.engine.call_fn_with_options(
            options,
            &mut Scope::new(),
            &self.ast,
            "update",
            vec![message, elevators, floors],
        )?;

        let lowered = self.call(api::LOWER, vec![commands])?;
        let commands = Command::decode(lowered)?;
        if !commands.is_empty() {
            self.snapshots = None;
        }
        for command in commands {
            command.apply(&mut self.world)?;
        }
        Ok(())
    }

    /// Snapshots of the world as it stands, built by the prelude's
    /// bridges.
    fn snapshot(&self) -> Result<Snapshots, Box<EvalAltResult>> {
        let elevators = self
            .world
            .elevators()
            .iter()
            .map(|elevator| self.call(api::ELEVATOR, api::elevator_arguments(elevator)))
            .collect::<Result<Array, _>>()?;
        let floors = self
            .world
            .floors()
            .iter()
            .map(|floor| self.call(api::FLOOR, api::floor_arguments(floor)))
            .collect::<Result<Array, _>>()?;
        Ok(Snapshots {
            elevators: Dynamic::from(elevators),
            floors: Dynamic::from(floors),
        })
    }

    /// Calls one of the prelude's bridge functions.
    fn call(&self, bridge: &str, arguments: Vec<Dynamic>) -> Result<Dynamic, Box<EvalAltResult>> {
        self.engine.call_fn_with_options(
            CallFnOptions::new().eval_ast(false),
            &mut Scope::new(),
            &self.ast,
            bridge,
            arguments,
        )
    }
}
