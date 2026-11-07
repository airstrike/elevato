//! rawr bindings over the core simulation: players write message-driven
//! Rust programs that steer the elevators - every world event arrives
//! as a plain-data `Message` enum, world state arrives as plain-data
//! snapshot structs, and the program answers with commands.
//!
//! The game API is itself Rust source - `prelude.rs` at the crate root -
//! appended to every player program before rawr compiles it.
//! [`Program::compile`] proves a source has the required shape - a
//! zero-parameter `fn new` whose return value is the model, and a
//! `fn update(model: &mut M, message: Message, elevators: &[Elevator],
//! floors: &[Floor]) -> Vec<Command>` over that same model type; a
//! [`Runtime`] is minted from a program plus a challenge and seed, owns
//! the resulting world directly, and replays the core driver contract
//! frame by frame.

mod api;
mod error;
mod runtime;

pub use error::Error;
pub use runtime::Runtime;

use rawr::{AST, Scope};

/// The game API every program is compiled against.
const PRELUDE: &str = include_str!("../prelude.rs");

/// A function that calls `new` and `update` exactly as the runtime does,
/// so rawr's own type check proves the program's shape: `update` takes
/// the model `new` returns, by `&mut`, plus the message and the two
/// snapshot slices, and returns the commands.
const WITNESS: &str = "
fn __elevato_witness(message: Message, elevators: &[Elevator], floors: &[Floor]) -> Vec<Command> {
    let mut model = new();
    update(&mut model, message, elevators, floors)
}
";

/// The signature `update` must have, for error messages.
pub(crate) const UPDATE_SIGNATURE: &str = "fn update(model: &mut Model, message: Message, \
                                           elevators: &[Elevator], floors: &[Floor]) -> Vec<Command>";

/// A compiled player program, proven to have the valid shape -
/// `fn new()` + `fn update` - and the only thing a [`Runtime`] can be
/// minted from (`smart-constructor-newtype`).
#[derive(Debug, Clone)]
pub struct Program {
    pub(crate) ast: AST,
}

impl Program {
    /// Compiles Rust source against the game API and verifies the
    /// program shape: `fn new()` (zero parameters; its return value is
    /// the model) and `fn update(model: &mut _, Message, &[Elevator],
    /// &[Floor]) -> Vec<Command>` over that model are both required.
    pub fn compile(source: &str) -> Result<Self, Error> {
        let engine = api::engine();
        // The player's text comes first, so rawr's line numbers are the
        // editor's; the newline keeps a trailing `//` comment from
        // swallowing the prelude's first line.
        match compile(&engine, &[source, "\n", PRELUDE, WITNESS]) {
            Ok(ast) => Ok(Self { ast }),
            // The witness failed - either the program itself does not
            // compile, or its entry points have the wrong shape. Compile
            // it without the witness to tell which.
            Err(witnessed) => {
                let ast = compile(&engine, &[source, "\n", PRELUDE])?;
                let arity = |name: &str| {
                    ast.iter_functions()
                        .find(|function| function.name == name)
                        .map(|function| function.params.len())
                };
                match (arity("new"), arity("update")) {
                    (None, _) => Err(Error::MissingNew),
                    (Some(arity @ 1..), _) => Err(Error::NewArity(arity)),
                    (Some(0), None) => Err(Error::MissingUpdate),
                    (Some(0), Some(_)) => Err(Error::Signature(witnessed)),
                }
            }
        }
    }
}

fn compile(engine: &rawr::Engine, parts: &[&str]) -> Result<AST, rawr::ParseError> {
    engine.compile_scripts_with_scope(&Scope::new(), parts)
}
