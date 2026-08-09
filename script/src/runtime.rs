//! The frame driver: owns the engine, the compiled AST, the world, and
//! the handler registry, and replays the core driver contract
//! ([`elevato_core::world`] module docs) one frame at a time.
//!
//! This deliberately duplicates the ~30-line substep/dispatch loop of
//! [`elevato_core::headless::run`] — the runtime cannot implement
//! [`elevato_core::controller::Controller`] because rhai handles are
//! `'static` registered types and cannot hold the `&mut World` the trait
//! passes around (see `.claude/DECISIONS.md` D6). The duplication is
//! pinned by the parity test in `tests/solutions.rs`: the rhai naive
//! solution must produce byte-identical final stats to a native
//! controller run through `core::headless`.

use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use elevato_core::World;
use elevato_core::challenge::{Challenge, Outcome};
use elevato_core::event::{Direction, Event};
use elevato_core::stats::Stats;
use elevato_core::world::DT_MAX;
use rhai::{AST, Array, CallFnOptions, Dynamic, Engine, FnPtr, Map, Scope};

use crate::api::{self, Binding, Hook, elevator, floor};
use crate::{Error, Mode, Program};

/// Hard cap on drain rounds per dispatch point, mirroring
/// [`elevato_core::headless`]: a handler cascade past this many rounds
/// has its remaining events deferred to the next dispatch point.
const DISPATCH_ROUNDS: usize = 128;

/// A running scripted simulation, minted from a [`Program`] plus a
/// challenge and seed (`transformation-method`): construction builds the
/// world, runs `init`, and fires the initial idle round, so a `Runtime`
/// that exists is already under way — [`Runtime::frame`] does the rest.
pub struct Runtime {
    engine: Engine,
    ast: AST,
    /// Persists across calls so top-level `let`s survive `init`; script
    /// functions cannot see it, but evaluation order stays faithful.
    scope: Scope<'static>,
    world: Rc<RefCell<World>>,
    registry: Rc<RefCell<api::Registry>>,
    /// The handle arrays passed to `init` and `update`.
    elevators: Array,
    floors: Array,
    mode: Mode,
    /// The TEA dialect's state, `this`-bound into every `update` call;
    /// [`Dynamic::UNIT`] in classic mode.
    model: Dynamic,
    /// Declared parameter counts per script function name (several
    /// entries when a name is overloaded by arity). Closure params
    /// include their captures, which arrive curried at call time —
    /// dispatch subtracts the curry to get the callable arity.
    arities: HashMap<String, Vec<usize>>,
}

impl std::fmt::Debug for Runtime {
    // Hand-written because `rhai::Engine` has no `Debug`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime")
            .field("world", &self.world)
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl Drop for Runtime {
    // A handler that captures a handle strings a strong-`Rc` cycle:
    // Registry → FnPtr → curried Handle → Registry (and pins the world
    // through the same handle). Clearing the bindings breaks it, so an
    // Apply-style teardown actually frees the old world.
    fn drop(&mut self) {
        self.registry.borrow_mut().clear();
    }
}

impl Runtime {
    /// Builds the world for `challenge`, evaluates the program's
    /// top-level statements, runs `init(elevators, floors)`, and fires
    /// the initial idle round exactly like the original's `world.init()`
    /// (and [`elevato_core::headless::run`]).
    pub fn new(program: Program, challenge: &Challenge, seed: u64) -> Result<Self, Error> {
        let Program { ast, mode } = program;

        let mut arities: HashMap<String, Vec<usize>> = HashMap::new();
        for function in ast.iter_functions() {
            arities
                .entry(function.name.to_string())
                .or_default()
                .push(function.params.len());
        }

        let world = Rc::new(RefCell::new(World::new(challenge, seed)));
        let registry = Rc::new(RefCell::new(api::Registry::default()));
        let elevators: Array = (0..challenge.elevator_count())
            .map(|index| {
                Dynamic::from(elevator::Handle::new(
                    world.clone(),
                    registry.clone(),
                    index,
                ))
            })
            .collect();
        let floors: Array = (0..challenge.floor_count())
            .map(|index| Dynamic::from(floor::Handle::new(world.clone(), registry.clone(), index)))
            .collect();

        if matches!(mode, Mode::Tea { .. }) {
            registry.borrow_mut().tea = true;
        }

        let mut runtime = Self {
            engine: api::engine(),
            ast,
            scope: Scope::new(),
            world,
            registry,
            elevators,
            floors,
            mode,
            model: Dynamic::UNIT,
            arities,
        };

        // Top-level statements run exactly once, here, before the boot
        // function — eval_ast(false) everywhere later keeps them from
        // re-running.
        let options = CallFnOptions::new().eval_ast(true).rewind_scope(false);
        match mode {
            Mode::Classic { .. } => {
                let _: Dynamic = runtime.engine.call_fn_with_options(
                    options,
                    &mut runtime.scope,
                    &runtime.ast,
                    "init",
                    (runtime.elevators.clone(), runtime.floors.clone()),
                )?;
            }
            Mode::Tea { model_takes_world } => {
                runtime.model = if model_takes_world {
                    runtime.engine.call_fn_with_options(
                        options,
                        &mut runtime.scope,
                        &runtime.ast,
                        "model",
                        (runtime.elevators.clone(), runtime.floors.clone()),
                    )?
                } else {
                    runtime.engine.call_fn_with_options(
                        options,
                        &mut runtime.scope,
                        &runtime.ast,
                        "model",
                        (),
                    )?
                };
            }
        }

        {
            let mut world = runtime.world.borrow_mut();
            for index in 0..world.elevators().len() {
                world.check_destination_queue(index);
            }
        }
        runtime.dispatch()?;
        Ok(runtime)
    }

    /// Advances one frame of `substeps` × [`DT_MAX`] simulated seconds:
    /// user `update` once with the whole frame dt (when defined), then
    /// per substep — step physics, dispatch drained events, process
    /// staged arrivals, dispatch again — breaking the moment the
    /// challenge decides. A returned error aborts the frame; the caller
    /// pauses on it.
    pub fn frame(&mut self, substeps: usize) -> Result<(), Error> {
        if self.world.borrow().ended() {
            return Ok(());
        }
        let dt = substeps as f64 * DT_MAX;
        match self.mode {
            Mode::Classic { has_update: true } => {
                let options = CallFnOptions::new().eval_ast(false);
                let _: Dynamic = self.engine.call_fn_with_options(
                    options,
                    &mut self.scope,
                    &self.ast,
                    "update",
                    (dt, self.elevators.clone(), self.floors.clone()),
                )?;
            }
            Mode::Classic { has_update: false } => {}
            Mode::Tea { .. } => {
                // Time is a message like any other.
                let mut message = Map::new();
                message.insert("kind".into(), "tick".into());
                message.insert("dt".into(), dt.into());
                self.deliver(message)?;
            }
        }
        for _ in 0..substeps {
            if self.world.borrow().ended() {
                break;
            }
            self.world.borrow_mut().step(DT_MAX);
            self.dispatch()?;
            self.world.borrow_mut().process_arrivals();
            self.dispatch()?;
        }
        Ok(())
    }

    /// The live statistics snapshot.
    pub fn stats(&self) -> Stats {
        *self.world.borrow().stats()
    }

    /// How the challenge stands.
    pub fn outcome(&self) -> Outcome {
        self.world.borrow().outcome()
    }

    /// Whether the challenge has decided.
    pub fn ended(&self) -> bool {
        self.world.borrow().ended()
    }

    /// Read access to the world, for rendering. The borrow must not be
    /// held across a call to [`Runtime::frame`].
    pub fn world(&self) -> Ref<'_, World> {
        self.world.borrow()
    }

    /// Drains the event queue to the bound handlers until quiescent
    /// (capped at [`DISPATCH_ROUNDS`]). Never holds a world or registry
    /// borrow while script code runs.
    fn dispatch(&mut self) -> Result<(), Error> {
        for _ in 0..DISPATCH_ROUNDS {
            let events = self.world.borrow_mut().drain_events();
            if events.is_empty() {
                return Ok(());
            }
            for event in events {
                self.dispatch_event(event)?;
            }
        }
        Ok(())
    }

    /// Routes one world event: classic mode invokes the bound handlers,
    /// TEA mode folds a message into the model through `update`.
    fn dispatch_event(&mut self, event: Event) -> Result<(), Error> {
        if matches!(self.mode, Mode::Tea { .. }) {
            let message = self.message(event);
            return self.deliver(message);
        }
        self.dispatch_bindings(event)
    }

    /// The message map for a world event: `kind` (the `Event` name in
    /// snake_case) plus that event's fields — handles for elevators and
    /// floors, plain values for the rest.
    fn message(&self, event: Event) -> Map {
        let mut message = Map::new();
        let mut put = |key: &str, value: Dynamic| {
            message.insert(key.into(), value);
        };
        match event {
            Event::Idle { elevator } => {
                put("kind", "idle".into());
                put("elevator", self.elevators[elevator].clone());
            }
            Event::FloorButtonPressed { elevator, floor } => {
                put("kind", "floor_button_pressed".into());
                put("elevator", self.elevators[elevator].clone());
                put("floor", (floor as i64).into());
            }
            Event::PassingFloor {
                elevator,
                floor,
                direction,
            } => {
                put("kind", "passing_floor".into());
                put("elevator", self.elevators[elevator].clone());
                put("floor", (floor as i64).into());
                put(
                    "direction",
                    match direction {
                        Direction::Up => "up".into(),
                        Direction::Down => "down".into(),
                    },
                );
            }
            Event::StoppedAtFloor { elevator, floor } => {
                put("kind", "stopped_at_floor".into());
                put("elevator", self.elevators[elevator].clone());
                put("floor", (floor as i64).into());
            }
            Event::UpButtonPressed { floor } => {
                put("kind", "up_button_pressed".into());
                put("floor", self.floors[floor].clone());
            }
            Event::DownButtonPressed { floor } => {
                put("kind", "down_button_pressed".into());
                put("floor", self.floors[floor].clone());
            }
        }
        message
    }

    /// Folds one message into the model: `update` runs with the model
    /// bound as `this`, so mutations persist across calls.
    fn deliver(&mut self, message: Map) -> Result<(), Error> {
        let mut full: Vec<Dynamic> = vec![
            Dynamic::from(message),
            Dynamic::from(self.elevators.clone()),
            Dynamic::from(self.floors.clone()),
        ];
        if let Some(arity) = self
            .arities
            .get("update")
            .and_then(|declared| declared.iter().copied().max())
        {
            full.truncate(arity);
            while full.len() < arity {
                full.push(Dynamic::UNIT);
            }
        }
        let mut model = std::mem::take(&mut self.model);
        let options = CallFnOptions::new()
            .eval_ast(false)
            .bind_this_ptr(&mut model);
        let result: Result<Dynamic, _> =
            self.engine
                .call_fn_with_options(options, &mut self.scope, &self.ast, "update", full);
        self.model = model;
        let _: Dynamic = result?;
        Ok(())
    }

    /// Invokes every handler bound to `event`, in registration order,
    /// with the arguments the original passed (research §2).
    fn dispatch_bindings(&self, event: Event) -> Result<(), Error> {
        let (hook, args): (Hook, Vec<Dynamic>) = match event {
            // The original's idle handlers receive nothing — closures
            // capture their elevator.
            Event::Idle { elevator } => (Hook::Idle { elevator }, Vec::new()),
            Event::FloorButtonPressed { elevator, floor } => (
                Hook::FloorButtonPressed { elevator },
                vec![Dynamic::from(floor as i64)],
            ),
            Event::PassingFloor {
                elevator,
                floor,
                direction,
            } => (
                Hook::PassingFloor { elevator },
                vec![
                    Dynamic::from(floor as i64),
                    match direction {
                        Direction::Up => "up".into(),
                        Direction::Down => "down".into(),
                    },
                ],
            ),
            Event::StoppedAtFloor { elevator, floor } => (
                Hook::StoppedAtFloor { elevator },
                vec![Dynamic::from(floor as i64)],
            ),
            // Floor handlers receive the floor handle, like the original
            // passed the floor object.
            Event::UpButtonPressed { floor } => (
                Hook::UpButtonPressed { floor },
                vec![self.floors[floor].clone()],
            ),
            Event::DownButtonPressed { floor } => (
                Hook::DownButtonPressed { floor },
                vec![self.floors[floor].clone()],
            ),
        };
        let bindings = self.registry.borrow().bindings(hook);
        for binding in &bindings {
            self.invoke(binding, &args)?;
        }
        Ok(())
    }

    /// Calls one handler, adapting the argument list to its arity: JS
    /// ignored extra arguments and passed `undefined` for missing ones,
    /// so the list is truncated — or padded with `()` — to what the
    /// handler declares (multi-event binds see the event name first).
    fn invoke(&self, binding: &Binding, args: &[Dynamic]) -> Result<(), Error> {
        let mut full: Vec<Dynamic> = Vec::with_capacity(args.len() + 1);
        if let Some(name) = &binding.prepended_name {
            full.push(name.clone().into());
        }
        full.extend(args.iter().cloned());
        if let Some(arity) = self.effective_arity(&binding.fn_ptr, full.len()) {
            full.truncate(arity);
            while full.len() < arity {
                full.push(Dynamic::UNIT);
            }
        }
        let _: Dynamic = binding.fn_ptr.call(&self.engine, &self.ast, full)?;
        Ok(())
    }

    /// The number of arguments a handler can be called with: declared
    /// parameters minus curried captures. With several arity overloads,
    /// the largest that `available` can satisfy wins (else the smallest,
    /// padded). `None` means the pointer targets something outside the
    /// AST (a native function) — call it with the full list.
    fn effective_arity(&self, fn_ptr: &FnPtr, available: usize) -> Option<usize> {
        let declared = self.arities.get(fn_ptr.fn_name())?;
        let curry = fn_ptr.curry().len();
        let mut best: Option<usize> = None;
        let mut smallest: Option<usize> = None;
        for candidate in declared
            .iter()
            .filter_map(|&params| params.checked_sub(curry))
        {
            if candidate <= available {
                best = Some(best.map_or(candidate, |b| b.max(candidate)));
            }
            smallest = Some(smallest.map_or(candidate, |s| s.min(candidate)));
        }
        best.or(smallest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropping_the_runtime_frees_the_world_despite_handler_capture_cycles() {
        // The idle handler captures its elevator handle, currying a
        // strong Rc chain back into the registry — the exact cycle the
        // Drop impl exists to break.
        let source = r#"
fn init(elevators, floors) {
    let e = elevators[0];
    e.on("idle", || e.go_to_floor(0));
}
"#;
        let program = crate::Program::compile(source).unwrap();
        let challenge = &elevato_core::challenge::roster()[0];
        let runtime = Runtime::new(program, challenge, 1).unwrap();
        let world = Rc::downgrade(&runtime.world);
        drop(runtime);
        assert!(
            world.upgrade().is_none(),
            "the world leaked after the runtime was dropped"
        );
    }
}
