//! Rhai bindings over the core simulation: players write Rhai programs
//! that drive the elevators through the same API surface the original
//! game exposed to JS (see `API.md` at the repo root for the full
//! mapping and the documented deviations).
//!
//! [`Program::compile`] proves a source has one of the two valid
//! shapes — classic (`fn init` + `on(...)` handlers) or TEA (`fn model`
//! + a `this`-bound `fn update` receiving events as messages); a
//! [`Runtime`] is minted from a program plus a challenge and seed, owns
//! the resulting world, and replays the core driver contract frame by
//! frame.

mod api;
mod error;
mod runtime;

pub use error::Error;
pub use runtime::Runtime;

use rhai::AST;

/// How a program receives the world's events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// The faithful port: `fn init` binds `on(...)` handlers, and an
    /// optional `fn update(dt, elevators, floors)` runs per frame.
    Classic {
        /// Whether a frame `update` exists.
        has_update: bool,
    },
    /// The message dialect: `fn model` boots the state, and a required
    /// `update` — bound to the model as `this` — receives every event,
    /// ticks included.
    Tea {
        /// Whether `model` takes `(elevators, floors)` or nothing.
        model_takes_world: bool,
    },
}

/// A compiled player program, proven to have one of the two valid
/// shapes — classic (`fn init`) or TEA (`fn model` + `fn update`) —
/// and the only thing a [`Runtime`] can be minted from
/// (`smart-constructor-newtype`).
#[derive(Debug, Clone)]
pub struct Program {
    pub(crate) ast: AST,
    pub(crate) mode: Mode,
}

impl Program {
    /// Compiles rhai source and verifies the program shape: classic
    /// requires `fn init(elevators, floors)` with `fn update(dt,
    /// elevators, floors)` optional; a program defining `fn model`
    /// instead is TEA and must define an `update` for its messages.
    pub fn compile(source: &str) -> Result<Self, Error> {
        let engine = api::engine();
        let ast = engine.compile(source)?;

        let mut has_init = false;
        let mut has_update = false;
        let mut model_arity = None;
        let mut bad_update_arity = None;
        for function in ast.iter_functions() {
            match (function.name, function.params.len()) {
                ("init", 2) => has_init = true,
                ("model", arity) => model_arity = Some(arity),
                ("update", 3) => has_update = true,
                ("update", arity) => bad_update_arity = Some(arity),
                _ => {}
            }
        }

        if let Some(arity) = model_arity {
            if has_init {
                return Err(Error::AmbiguousMode);
            }
            let model_takes_world = match arity {
                0 => false,
                2 => true,
                arity => return Err(Error::ModelArity(arity)),
            };
            if !has_update && bad_update_arity.is_none() {
                return Err(Error::MissingUpdate);
            }
            return Ok(Self {
                ast,
                mode: Mode::Tea { model_takes_world },
            });
        }

        if !has_init {
            return Err(Error::MissingInit);
        }
        if let Some(arity) = bad_update_arity {
            if !has_update {
                return Err(Error::UpdateArity(arity));
            }
        }
        Ok(Self {
            ast,
            mode: Mode::Classic { has_update },
        })
    }
}
