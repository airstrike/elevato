//! Everything that can go wrong compiling or running a player program.

/// A compile-time or runtime failure in a player program. Runtime errors
/// abort the frame that raised them; the app pauses on one, faithful to
/// the original's "There is a problem with your code".
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The source failed to parse; the inner error carries the position.
    #[error("compile error: {0}")]
    Compile(#[from] rhai::ParseError),

    /// The program never defines the boot function whose return value
    /// becomes the model.
    #[error("the program must define `fn new()` — its return value is the model")]
    MissingNew,

    /// A `fn new` exists, but with parameters it cannot have.
    #[error("`fn new` takes no parameters, found {0}")]
    NewArity(usize),

    /// No `fn update` to consume the messages.
    #[error("the program must define `fn update(message, elevators, floors)`")]
    MissingUpdate,

    /// A throw or failure inside `new` or `update` — including a
    /// command applied to an elevator the challenge does not have, and
    /// an `update` return value that is not a command. The inner
    /// error's display includes the source position when there is one.
    #[error("{0}")]
    Runtime(#[from] Box<rhai::EvalAltResult>),
}
