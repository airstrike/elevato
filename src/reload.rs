//! What becomes of an applied edit: whether rawr runs the player's
//! program ([`Host`]), and what rawr did with each save ([`Outcome`]).
//!
//! rawr decides every outcome. elevato never checks an edit's
//! compatibility itself: it writes the file rawr watches and reports what
//! rawr says, through the `rawr-reload` crate. Under `cargo run` and on
//! the web the program is compiled into the binary, so an edit can't
//! reach the running game at all.

use iced::Subscription;

/// How the player's program runs, which decides what an applied edit can
/// do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    /// `rawr run` interprets the program, reloads it on every save, and
    /// says what each reload did.
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

/// A save rawr refused: its message, and where in the program it points
/// when it points somewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    message: String,
    at: Option<Position>,
}

/// A 1-based line and column in the program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: u32,
    pub column: u32,
}

/// How this process runs the player's program.
pub fn host() -> Host {
    #[cfg(target_arch = "wasm32")]
    {
        Host::Compiled
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if rawr_reload::running() {
            Host::Rawr
        } else {
            Host::Compiled
        }
    }
}

impl Host {
    /// rawr's outcomes for the saves it sees, from any editor, in order;
    /// none for a compiled program.
    pub fn outcomes(self) -> Subscription<Outcome> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Rawr => Subscription::run(listen),
            #[cfg(target_arch = "wasm32")]
            Self::Rawr => Subscription::none(),
            Self::Compiled => Subscription::none(),
        }
    }
}

/// rawr's reload stream, read on a thread of its own: each outcome blocks
/// until rawr says what a save did.
#[cfg(not(target_arch = "wasm32"))]
fn listen() -> iced::futures::channel::mpsc::UnboundedReceiver<Outcome> {
    let (sender, receiver) = iced::futures::channel::mpsc::unbounded();
    if let Some(outcomes) = rawr_reload::outcomes() {
        std::thread::spawn(move || {
            for outcome in outcomes {
                if sender.unbounded_send(Outcome::from(outcome)).is_err() {
                    break;
                }
            }
        });
    }
    receiver
}

#[cfg(not(target_arch = "wasm32"))]
impl From<rawr_reload::Outcome> for Outcome {
    fn from(outcome: rawr_reload::Outcome) -> Self {
        match outcome {
            rawr_reload::Outcome::Restarted { reason } => Self::Restarted(reason),
            rawr_reload::Outcome::Queued => Self::Queued,
            rawr_reload::Outcome::Swapped => Self::Swapped,
            rawr_reload::Outcome::Failed { message, at } => Self::Failed(Failure {
                message,
                at: at.map(|span| Position {
                    line: span.line,
                    column: span.column,
                }),
            }),
            rawr_reload::Outcome::Restarting { reason } => Self::Restarting(reason),
        }
    }
}

impl Failure {
    /// A refusal of `message`, pointing `at` a place in the program when
    /// rawr says where.
    pub fn new(message: impl Into<String>, at: Option<Position>) -> Self {
        Self {
            message: message.into(),
            at,
        }
    }

    /// What the check said.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Where it points.
    pub fn at(&self) -> Option<Position> {
        self.at
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn each_rawr_outcome_maps_onto_its_own() {
        let parse = |line| Outcome::from(rawr_reload::Outcome::parse(line).unwrap());
        assert_eq!(parse(r#"{"event":"queued"}"#), Outcome::Queued);
        assert_eq!(parse(r#"{"event":"swapped"}"#), Outcome::Swapped);
        assert_eq!(
            parse(r#"{"event":"restarted","reason":"`Program` changed its fields"}"#),
            Outcome::Restarted("`Program` changed its fields".to_string())
        );
        assert_eq!(
            parse(r#"{"event":"restarting","reason":"`Program` changed its fields"}"#),
            Outcome::Restarting("`Program` changed its fields".to_string())
        );
        assert_eq!(
            parse(
                r#"{"event":"failed","message":"cannot find value `nxt`","file":"main.rs","line":51,"column":33}"#
            ),
            Outcome::Failed(Failure::new(
                "cannot find value `nxt`",
                Some(Position {
                    line: 51,
                    column: 33
                })
            ))
        );
        assert_eq!(
            parse(r#"{"event":"failed","message":"the build failed"}"#),
            Outcome::Failed(Failure::new("the build failed", None))
        );
    }
}
