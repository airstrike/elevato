//! Everything that can go wrong compiling or running a player program.

/// A compile-time or runtime failure in a player program. Runtime errors
/// abort the frame that raised them; the app pauses on one, faithful to
/// the original's "There is a problem with your code".
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The source failed to parse; the inner error carries the position.
    #[error("compile error: {0}")]
    Compile(#[from] rhai::ParseError),

    /// The program never defines the required entry point.
    #[error("the program must define `fn init(elevators, floors)`")]
    MissingInit,

    /// A `fn update` exists, but not with the three required parameters —
    /// silently never calling it would be crueler than refusing.
    #[error("`fn update` must take (dt, elevators, floors), found {0} parameter(s)")]
    UpdateArity(usize),

    /// A throw or failure inside `init`, `update`, or an event handler.
    /// The inner error's display includes the source position.
    #[error("{0}")]
    Runtime(#[from] Box<rhai::EvalAltResult>),
}
