//! The frame driver: owns the engine, the compiled AST, the model, and
//! the world - directly, by value - and replays the core driver
//! contract ([`elevato_core::world`] module docs) one frame at a time.
//!
//! This deliberately duplicates the ~30-line substep/dispatch loop of
//! [`elevato_core::headless::run`] - the runtime cannot implement
//! [`elevato_core::controller::Controller`] because the trait's hooks
//! are infallible while a script error must abort the frame and pause
//! playback (see `.claude/DECISIONS.md` D6/D9). The duplication is
//! pinned by the parity test in `tests/solutions.rs`: the rhai naive
//! solution must produce byte-identical final stats to a native
//! controller run through `core::headless`.
//!
//! Scripts never touch the world. Per message the runtime builds fresh
//! snapshots, calls `update` with the model bound as `this`, and applies
//! the returned commands immediately - before the next message
//! dispatches - so a command cascade unfolds in exactly the order the
//! original's synchronous callbacks did.

use elevato_core::World;
use elevato_core::challenge::{Challenge, Outcome};
use elevato_core::event::Event;
use elevato_core::stats::Stats;
use elevato_core::world::DT_MAX;
use rhai::{AST, Array, CallFnOptions, Dynamic, Engine, Scope};

use crate::api::{self, Command};
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
    /// Persists across calls so top-level `let`s survive the boot;
    /// script functions cannot see it, but evaluation order stays
    /// faithful.
    scope: Scope<'static>,
    world: World,
    /// The value `fn new` returned, bound as `this` into every `update`
    /// call; mutations persist.
    model: Dynamic,
    /// Largest declared arity of `fn update`; the `[message, elevators,
    /// floors]` argument list is truncated or padded to it.
    update_arity: usize,
}

impl std::fmt::Debug for Runtime {
    // Hand-written because `rhai::Engine` has no `Debug`.
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
        let Program { ast, update_arity } = program;
        let mut runtime = Self {
            engine: api::engine(),
            ast,
            scope: Scope::new(),
            world: World::new(challenge, seed),
            model: Dynamic::UNIT,
            update_arity,
        };

        // Top-level statements run exactly once, here, before the boot
        // function - eval_ast(false) everywhere later keeps them from
        // re-running.
        let options = CallFnOptions::new().eval_ast(true).rewind_scope(false);
        runtime.model = runtime.engine.call_fn_with_options(
            options,
            &mut runtime.scope,
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
    /// a `tick` message with the whole frame dt first, then per substep -
    /// step physics, dispatch drained events, process staged arrivals,
    /// dispatch again - breaking the moment the challenge decides. A
    /// returned error aborts the frame; the caller pauses on it.
    pub fn frame(&mut self, substeps: usize) -> Result<(), Error> {
        if self.world.ended() {
            return Ok(());
        }
        // Time is a message like any other.
        let dt = substeps as f64 * DT_MAX;
        self.deliver(api::Message::Tick { dt })?;

        for _ in 0..substeps {
            if self.world.ended() {
                break;
            }
            self.world.step(DT_MAX);
            self.dispatch()?;
            self.world.process_arrivals();
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
                self.deliver(message(event))?;
            }
        }
        Ok(())
    }

    /// Folds one message into the model and applies the returned
    /// commands: snapshots are rebuilt fresh, `update` runs with the
    /// model bound as `this` (mutations persist), and the return value
    /// is applied before this call returns - i.e. before the next
    /// message dispatches.
    fn deliver(&mut self, message: api::Message) -> Result<(), Error> {
        let mut arguments: Vec<Dynamic> = vec![
            Dynamic::from(message),
            Dynamic::from(api::elevator_snapshots(&self.world)),
            Dynamic::from(api::floor_snapshots(&self.world)),
        ];
        arguments.truncate(self.update_arity);
        while arguments.len() < self.update_arity {
            arguments.push(Dynamic::UNIT);
        }

        let mut model = std::mem::take(&mut self.model);
        let options = CallFnOptions::new()
            .eval_ast(false)
            .bind_this_ptr(&mut model);
        let result: Result<Dynamic, _> = self.engine.call_fn_with_options(
            options,
            &mut self.scope,
            &self.ast,
            "update",
            arguments,
        );
        self.model = model;
        self.apply(result?)
    }

    /// Interprets `update`'s return value - the effect channel: `()` is
    /// nothing (an unmatched `switch` arm lands here), a command
    /// applies, an array applies each command element in order (unit
    /// elements are skipped). Anything else is a runtime error naming
    /// the type.
    fn apply(&mut self, returned: Dynamic) -> Result<(), Error> {
        if returned.is_unit() {
            Ok(())
        } else if returned.is_array() {
            for element in returned.cast::<Array>() {
                if !element.is_unit() {
                    self.apply_command(element)?;
                }
            }
            Ok(())
        } else {
            self.apply_command(returned)
        }
    }

    /// Applies one returned value as a command, or refuses by type name.
    fn apply_command(&mut self, value: Dynamic) -> Result<(), Error> {
        if value.type_id() == std::any::TypeId::of::<Command>() {
            let command = value
                .try_cast::<Command>()
                .expect("invariant: the type id was just checked");
            Ok(command.apply(&mut self.world)?)
        } else {
            Err(api::runtime_error(format!(
                "update returned a value of type {} - return a command, an array of commands, \
                 or nothing",
                value.type_name()
            ))
            .into())
        }
    }
}

/// The [`api::Message`] for a world event - all plain data, elevators
/// and floors as indices.
fn message(event: Event) -> api::Message {
    match event {
        Event::Idle { elevator } => api::Message::Idle { elevator },
        Event::FloorButtonPressed { elevator, floor } => {
            api::Message::FloorButtonPressed { elevator, floor }
        }
        Event::PassingFloor {
            elevator,
            floor,
            direction,
        } => api::Message::PassingFloor {
            elevator,
            floor,
            direction,
        },
        Event::StoppedAtFloor { elevator, floor } => {
            api::Message::StoppedAtFloor { elevator, floor }
        }
        Event::UpButtonPressed { floor } => api::Message::UpButtonPressed { floor },
        Event::DownButtonPressed { floor } => api::Message::DownButtonPressed { floor },
    }
}
