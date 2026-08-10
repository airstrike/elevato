//! Rhai bindings over the core simulation: players write message-driven
//! Rhai programs that steer the elevators - every world event arrives
//! as a plain-data message, world state arrives as plain-data
//! snapshots, and the program answers with commands.
//!
//! [`Program::compile`] proves a source has the required shape - a
//! zero-parameter `fn new` whose return value is the model, and a
//! `fn update`, bound to that model as `this`, that receives the
//! messages; a [`Runtime`] is minted from a program plus a challenge
//! and seed, owns the resulting world directly, and replays the core
//! driver contract frame by frame.

mod api;
mod error;
mod runtime;

pub use error::Error;
pub use runtime::Runtime;

use rhai::AST;

/// A compiled player program, proven to have the valid shape -
/// `fn new()` + `fn update` - and the only thing a [`Runtime`] can be
/// minted from (`smart-constructor-newtype`).
#[derive(Debug, Clone)]
pub struct Program {
    pub(crate) ast: AST,
    /// Largest declared arity of `fn update`; the runtime truncates or
    /// pads the `[message, elevators, floors]` argument list to it.
    pub(crate) update_arity: usize,
}

impl Program {
    /// Compiles rhai source and verifies the program shape: `fn new()`
    /// (zero parameters; its return value is the model) and `fn update`
    /// (any arity up to `(message, elevators, floors)`) are both
    /// required.
    pub fn compile(source: &str) -> Result<Self, Error> {
        let engine = api::engine();
        let ast = engine.compile(source)?;

        let mut new_arity = None;
        let mut update_arity = None;
        for function in ast.iter_functions() {
            let arity = function.params.len();
            if function.name == "new" {
                // With several overloads, the zero-parameter one is the
                // boot function and the others are ordinary helpers.
                new_arity = Some(match new_arity {
                    Some(0) => 0,
                    _ => arity,
                });
            } else if function.name == "update" {
                update_arity = Some(update_arity.map_or(arity, |max: usize| max.max(arity)));
            }
        }

        match new_arity {
            None => Err(Error::MissingNew),
            Some(arity @ 1..) => Err(Error::NewArity(arity)),
            Some(0) => match update_arity {
                None => Err(Error::MissingUpdate),
                Some(update_arity) => Ok(Self { ast, update_arity }),
            },
        }
    }
}
