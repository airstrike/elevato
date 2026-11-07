//! Everything that can go wrong compiling or running a player program.

use crate::UPDATE_SIGNATURE;

/// A compile-time or runtime failure in a player program. Runtime errors
/// abort the frame that raised them; the app pauses on one, faithful to
/// the original's "There is a problem with your code".
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The source failed rawr's check; the inner error prints as rustc
    /// prints it, with the code, the message and the source line.
    #[error("{0}")]
    Compile(#[from] rawr::ParseError),

    /// The program never defines the boot function whose return value
    /// becomes the model.
    #[error("the program must define `fn new() -> Model` - its return value is the model")]
    MissingNew,

    /// A `fn new` exists, but with parameters it cannot have.
    #[error("`fn new` takes no parameters, found {0}")]
    NewArity(usize),

    /// No `fn update` to consume the messages.
    #[error("the program must define `{UPDATE_SIGNATURE}`")]
    MissingUpdate,

    /// `new` and `update` exist, but `update` does not take the model
    /// `new` returns, or its parameters or return type differ from the
    /// game's. The inner error is rawr's type error at the call the
    /// runtime makes.
    #[error("`update` must be `{UPDATE_SIGNATURE}`, where `Model` is the type `new` returns\n{0}")]
    Signature(rawr::ParseError),

    /// A panic or failure inside `new` or `update` - including a
    /// command applied to an elevator the challenge does not have. The
    /// inner error's display includes the source position when there is
    /// one.
    #[error("{0}")]
    Runtime(#[from] Box<rawr::EvalAltResult>),
}
