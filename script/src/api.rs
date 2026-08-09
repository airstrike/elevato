//! The scripting API surface: engine construction and the handler
//! registry the `on(...)` methods write into.
//!
//! Handles ([`elevator::Handle`], [`floor::Handle`]) are cheap clonable
//! `(Rc<RefCell<World>>, index)` references registered as rhai types.
//! Every handle method borrows the `RefCell` only for the duration of
//! that one call — rhai invocations are strictly sequential, so no
//! borrow is ever live while script code runs. The original's reentrant
//! synchronous events (`goToFloor` → `idle` → handler → `goToFloor`) are
//! reproduced by the runtime's drain-to-quiescence dispatch loop
//! instead: a command stages events in the world's queue during its own
//! short borrow, and the runtime drains them after the borrow ends.

pub(crate) mod elevator;
pub(crate) mod floor;

use std::collections::HashMap;

use rhai::{Engine, EvalAltResult, FnPtr, ImmutableString, Position};

/// A position-less runtime error for fallible API methods (rhai adds the
/// call-site position when it propagates).
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
    // bodies, 8 vs 64 call levels) — tight enough that a realistic
    // program fails to parse in debug only. Pin generous values so a
    // program compiles and runs identically in both profiles. No
    // operations limit: per-update replanning strategies must never trip
    // a budget the original did not have.
    engine.set_max_expr_depths(128, 128);
    engine.set_max_call_levels(64);
    elevator::register(&mut engine);
    floor::register(&mut engine);
    engine
}

/// One handler slot: which event on which elevator or floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Hook {
    /// `elevator.on("idle", …)`.
    Idle { elevator: usize },
    /// `elevator.on("floor_button_pressed", …)`.
    FloorButtonPressed { elevator: usize },
    /// `elevator.on("passing_floor", …)`.
    PassingFloor { elevator: usize },
    /// `elevator.on("stopped_at_floor", …)`.
    StoppedAtFloor { elevator: usize },
    /// `floor.on("up_button_pressed", …)`.
    UpButtonPressed { floor: usize },
    /// `floor.on("down_button_pressed", …)`.
    DownButtonPressed { floor: usize },
}

/// One registered handler: the function pointer plus, for multi-event
/// binds, the event name riot prepended as the first handler argument
/// (single-event binds pass only the event's own arguments).
#[derive(Debug, Clone)]
pub(crate) struct Binding {
    /// The handler; captured variables ride along as curried arguments.
    pub fn_ptr: FnPtr,
    /// `Some(event_name)` when the bind came from a space-separated
    /// multi-event string.
    pub prepended_name: Option<ImmutableString>,
}

/// Every `on(...)` binding, keyed by hook. Written by handles during
/// `init` (or later — handlers may bind more handlers), read by the
/// runtime at dispatch.
#[derive(Debug, Default)]
pub(crate) struct Registry {
    bindings: HashMap<Hook, Vec<Binding>>,
    /// Set for TEA programs: `on(...)` refuses to bind, since events
    /// arrive through `fn update(message, …)` instead.
    pub(crate) tea: bool,
}

impl Registry {
    /// Appends a binding; a hook's handlers dispatch in registration
    /// order, like riot's observable.
    pub(crate) fn bind(&mut self, hook: Hook, binding: Binding) {
        self.bindings.entry(hook).or_default().push(binding);
    }

    /// A hook's bindings, cloned out — dispatch must never hold the
    /// registry borrow while invoking rhai (a handler may bind more
    /// handlers).
    pub(crate) fn bindings(&self, hook: Hook) -> Vec<Binding> {
        self.bindings.get(&hook).cloned().unwrap_or_default()
    }

    /// Drops every binding. The runtime calls this on teardown to break
    /// the `Registry → FnPtr → curried Handle → Registry` reference
    /// cycle (handles hold strong `Rc`s; a handler capturing its
    /// elevator would otherwise pin the registry — and the world —
    /// forever).
    pub(crate) fn clear(&mut self) {
        self.bindings.clear();
    }
}
