//! What becomes of an applied edit: whether rawr runs the player's program
//! ([`Host`]), and what rawr did with each save ([`Outcome`]).
//!
//! rawr decides every outcome. elevato never checks an edit's compatibility
//! itself: it writes the file rawr watches and reports what rawr says, through
//! the `rawr-reload` crate. On the web the page runs the program on rawr's
//! universe: it compiles each save and either restarts the game with it or
//! hands back the compiler's error. Under `cargo run` the program is compiled
//! into the binary, so an edit can't reach the running game at all.

use iced::Subscription;

/// How the player's program runs, which decides what an applied edit can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    /// `rawr run` interprets the program, reloads it on every save, and says
    /// what each reload did.
    Rawr,
    /// The web page runs the program on rawr's universe. A save that compiles
    /// restarts the game with it; one that doesn't comes back as its error.
    Page,
    /// `cargo run` compiled the program in; edits wait for the next build.
    Compiled,
}

/// What rawr did with a save, in the order it happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// rawr restarted this process with the program's new code, for this
    /// one-line reason. Only ever the first outcome, after an edit that rawr
    /// couldn't keep the program's state across.
    Restarted(String),
    /// The save compiled and waits for the program's next message, the first
    /// call from the game into the new code.
    Queued,
    /// The new code is in and the program kept its state.
    Swapped,
    /// rawr can't keep the program's state across this edit and restarts the
    /// game with the new code, for this one-line reason.
    Restarting(String),
    /// The save didn't compile; the last good program keeps running.
    Failed(Failure),
}

/// A save rawr refused: its message, and where in the program it points when it
/// points somewhere.
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
        if page::running() {
            Host::Page
        } else {
            Host::Compiled
        }
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
    /// rawr's outcomes for the saves it sees, from any editor, in order; none
    /// for a compiled program.
    pub fn outcomes(self) -> Subscription<Outcome> {
        match self {
            #[cfg(not(target_arch = "wasm32"))]
            Self::Rawr => Subscription::run(listen),
            #[cfg(target_arch = "wasm32")]
            Self::Rawr => Subscription::none(),
            #[cfg(target_arch = "wasm32")]
            Self::Page => Subscription::run(page::listen),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Page => Subscription::none(),
            Self::Compiled => Subscription::none(),
        }
    }

    /// Apply the saved `source`, once it is written. rawr sees the write
    /// itself; the page is handed the source.
    pub fn apply(self, source: &str) {
        match self {
            #[cfg(target_arch = "wasm32")]
            Self::Page => page::apply(source),
            #[cfg(not(target_arch = "wasm32"))]
            Self::Page => {
                let _ = source;
            }
            Self::Rawr | Self::Compiled => {}
        }
    }
}

/// The page that runs this game on rawr's universe, which sets the `elevato`
/// global on the game's window before it starts it: `{ restarted: bool,
/// failed?: string }`. The game hands the page each save as `{ elevato:
/// "apply", source }`; the page answers a save that doesn't compile with `{
/// elevato: "failed", text }`.
#[cfg(target_arch = "wasm32")]
mod page {
    use iced::futures::channel::mpsc;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::{JsCast, JsValue};

    use super::{Failure, Outcome};

    /// The global the page sets.
    const GLOBAL: &str = "elevato";

    /// Whether the page set the global.
    pub fn running() -> bool {
        global().is_some()
    }

    fn global() -> Option<JsValue> {
        let window = web_sys::window()?;
        let value = js_sys::Reflect::get(&window, &GLOBAL.into()).ok()?;
        value.is_object().then_some(value)
    }

    fn field(object: &JsValue, name: &str) -> Option<JsValue> {
        js_sys::Reflect::get(object, &name.into())
            .ok()
            .filter(|value| !value.is_undefined() && !value.is_null())
    }

    /// Hand `source` to the page.
    pub fn apply(source: &str) {
        let Some(parent) = web_sys::window().and_then(|window| window.parent().ok().flatten())
        else {
            return;
        };
        let message = js_sys::Object::new();
        let _ = js_sys::Reflect::set(&message, &GLOBAL.into(), &"apply".into());
        let _ = js_sys::Reflect::set(&message, &"source".into(), &source.into());
        let _ = parent.post_message(&message, "*");
    }

    /// The page's outcomes: `Restarted` first when the page restarted the game
    /// for a save, then the failure it started with, then each save it answered
    /// with an error.
    pub fn listen() -> mpsc::UnboundedReceiver<Outcome> {
        let (sender, receiver) = mpsc::unbounded();
        let Some(window) = web_sys::window() else {
            return receiver;
        };
        if let Some(global) = global() {
            if field(&global, "restarted").is_some_and(|value| value.is_truthy()) {
                let _ = sender.unbounded_send(Outcome::Restarted(String::from(
                    "the page ran the saved program",
                )));
            }
            if let Some(text) = field(&global, "failed").and_then(|value| value.as_string()) {
                let _ = sender.unbounded_send(Outcome::Failed(Failure::parse(&text)));
            }
        }
        let listener = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
            move |event: web_sys::MessageEvent| {
                let data = event.data();
                let failed = field(&data, GLOBAL)
                    .and_then(|kind| kind.as_string())
                    .is_some_and(|kind| kind == "failed");
                if let Some(text) = field(&data, "text")
                    .and_then(|text| text.as_string())
                    .filter(|_| failed)
                {
                    let _ = sender.unbounded_send(Outcome::Failed(Failure::parse(&text)));
                }
            },
        );
        let _ =
            window.add_event_listener_with_callback("message", listener.as_ref().unchecked_ref());
        // The listener lives as long as the game's window.
        listener.forget();
        receiver
    }
}

/// rawr's reload stream, read on a thread of its own: each outcome blocks until
/// rawr says what a save did.
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
    /// A refusal of `message`, pointing `at` a place in the program when rawr
    /// says where.
    pub fn new(message: impl Into<String>, at: Option<Position>) -> Self {
        Self {
            message: message.into(),
            at,
        }
    }

    /// The refusal a compiler's error `text` gives: its first error's message,
    /// at the first place it points to, as rustc and rawr write them
    /// (`error[E0425]: cannot find value` and ` --> src/main.rs:51:33` or ` -->
    /// src/main.rs:51:33-51:36`). Text in another shape is the message as it
    /// stands.
    pub fn parse(text: &str) -> Self {
        let mut lines = text.lines();
        let headline = lines.by_ref().find_map(|line| {
            let rest = line.strip_prefix("error")?;
            let rest = match rest.strip_prefix('[') {
                Some(code) => code.split_once(']')?.1,
                None => rest,
            };
            rest.strip_prefix(": ")
        });
        let Some(message) = headline else {
            return Self::new(text.trim(), None);
        };
        let at = lines
            .take_while(|line| !line.starts_with("error"))
            .find_map(|line| line.trim_start().strip_prefix("--> "))
            .and_then(|place| {
                // rawr writes a span, `main.rs:59:17-59:22`; its start is the
                // place.
                let place = place.trim();
                let place = match place.rsplit_once('-') {
                    Some((start, end))
                        if end.split(':').all(|part| part.parse::<u32>().is_ok()) =>
                    {
                        start
                    }
                    _ => place,
                };
                let mut parts = place.rsplitn(3, ':');
                let column = parts.next()?.parse().ok()?;
                let line = parts.next()?.parse().ok()?;
                Some(Position { line, column })
            });
        Self::new(message.trim(), at)
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

    #[test]
    fn a_compiler_error_parses_to_its_message_and_place() {
        let text = "error[E0425]: cannot find value `nxt` in this scope\n  \
                    --> /universe/src/main.rs:51:33\n   |\n51 |     nxt\n   |     ^^^\n\n\
                    error: aborting due to 1 previous error\n";
        assert_eq!(
            Failure::parse(text),
            Failure::new(
                "cannot find value `nxt` in this scope",
                Some(Position {
                    line: 51,
                    column: 33
                })
            )
        );
        assert_eq!(
            Failure::parse("error: expected `;`\n --> src/main.rs:3:9\n"),
            Failure::new("expected `;`", Some(Position { line: 3, column: 9 }))
        );
        assert_eq!(
            Failure::parse(
                "error[RAWR-RESOLVE-SELF]: `self` is only available inside an impl method\n \
                 --> /universe/src/main.rs:59:17-59:22\n"
            ),
            Failure::new(
                "`self` is only available inside an impl method",
                Some(Position {
                    line: 59,
                    column: 17
                })
            )
        );
        assert_eq!(
            Failure::parse("error: unsupported\n\nerror: aborting\n --> src/main.rs:1:1"),
            Failure::new("unsupported", None)
        );
        assert_eq!(
            Failure::parse("  the runner stopped \n"),
            Failure::new("the runner stopped", None)
        );
    }
}
