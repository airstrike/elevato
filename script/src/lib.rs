//! Rhai bindings over the core simulation: players write Rhai programs
//! that drive the elevators through the same API surface the original
//! game exposed to JS (see `API.md` at the repo root for the full
//! mapping and the documented deviations).
//!
//! [`Program::compile`] proves a source defines `fn init(elevators,
//! floors)`; a [`Runtime`] is minted from a program plus a challenge and
//! seed, owns the resulting world, and replays the core driver contract
//! frame by frame, dispatching world events to the program's `on(...)`
//! handlers.

mod api;
mod error;
mod runtime;

pub use error::Error;
pub use runtime::Runtime;

use rhai::AST;

/// A compiled player program, proven to define `fn init(elevators,
/// floors)` — and, when an `update` exists at all, a well-formed
/// `fn update(dt, elevators, floors)`. The only thing a [`Runtime`] can
/// be minted from (`smart-constructor-newtype`).
#[derive(Debug, Clone)]
pub struct Program {
    pub(crate) ast: AST,
    pub(crate) has_update: bool,
}

impl Program {
    /// Compiles rhai source and verifies the program shape. `fn
    /// init(elevators, floors)` is required; `fn update(dt, elevators,
    /// floors)` is optional (a documented deviation — the original
    /// required both).
    pub fn compile(source: &str) -> Result<Self, Error> {
        let engine = api::engine();
        let ast = engine.compile(source)?;

        let mut has_init = false;
        let mut has_update = false;
        let mut bad_update_arity = None;
        for function in ast.iter_functions() {
            match (function.name, function.params.len()) {
                ("init", 2) => has_init = true,
                ("update", 3) => has_update = true,
                ("update", arity) => bad_update_arity = Some(arity),
                _ => {}
            }
        }

        if !has_init {
            return Err(Error::MissingInit);
        }
        if let Some(arity) = bad_update_arity {
            if !has_update {
                return Err(Error::UpdateArity(arity));
            }
        }
        Ok(Self { ast, has_update })
    }
}
