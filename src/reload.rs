//! What becomes of an applied edit: whether rawr runs the player's
//! program ([`Host`]), and what rawr did with each save ([`Outcome`]).
//!
//! rawr decides every outcome. elevato never checks an edit's
//! compatibility itself: it writes the file rawr watches and reports what
//! rawr says. Under `cargo run` and on the web the program is compiled
//! into the binary, so an edit can't reach the running game at all.

use iced::Subscription;

/// How the player's program runs, which decides what an applied edit can
/// do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    /// `rawr run` interprets the program, reloads it on every save, and
    /// reports each reload's outcome on the socket `RAWR_RELOAD_EVENTS`
    /// names (rawr#1166).
    Rawr,
    /// `cargo run` or the web build compiled the program in; edits wait
    /// for the next build.
    Compiled,
}

/// What rawr did with a save, in the order it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// rawr restarted this process with the program's new code, for this
    /// one-line reason. Only ever the first outcome, after an edit that
    /// rawr couldn't keep the program's state across.
    Restarted(String),
    /// The save compiled and waits for the program's next message, the
    /// first call from the game into the new code.
    Queued,
    /// The new code is in and the program kept its state.
    Swapped,
    /// rawr can't keep the program's state across this edit and restarts
    /// the game with the new code, for this one-line reason.
    Restarting(String),
    /// The save didn't compile; the last good program keeps running.
    Failed(Failure),
}

/// A save rawr refused: its message, and where in the program it points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    message: String,
    line: usize,
    column: usize,
}

/// How this process runs the player's program.
pub fn host() -> Host {
    #[cfg(target_arch = "wasm32")]
    {
        Host::Compiled
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if std::env::var_os("RAWR_RELOAD_EVENTS").is_some_and(|socket| !socket.is_empty()) {
            Host::Rawr
        } else {
            Host::Compiled
        }
    }
}

impl Host {
    /// rawr's outcomes for the saves it sees, from any editor, in
    /// order; none for a compiled program.
    ///
    /// Empty under rawr too for now: rawr#1166 ships the `rawr-reload`
    /// crate whose blocking `outcomes()` iterator this runs, one
    /// [`Outcome`] per event.
    pub fn outcomes(self) -> Subscription<Outcome> {
        match self {
            Self::Rawr | Self::Compiled => Subscription::none(),
        }
    }
}

impl Failure {
    /// A refusal of `message` at 1-based `line` and `column`.
    pub fn new(message: impl Into<String>, line: usize, column: usize) -> Self {
        Self {
            message: message.into(),
            line,
            column,
        }
    }

    /// What the check said.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The 1-based line it points at.
    pub fn line(&self) -> usize {
        self.line
    }

    /// The 1-based column it points at.
    pub fn column(&self) -> usize {
        self.column
    }
}
