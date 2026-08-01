//! The application: a splash screen that hands off — via a measured
//! viewport width — to the game screen: challenge toolbar, stats bar,
//! sim canvas, and the editor pane, a thin TEA shell over
//! [`crate::playback::Playback`] composed with the [`crate::editor`]
//! cell.

use iced::keyboard::{self, key::Named};
use iced::time::{self, milliseconds};
use iced::widget::{
    Id, button, canvas, center, column, container, pick_list, row, selector, space, stack, text,
};
use iced::{Center, Element, Fill, Pixels, Subscription, Task};

use crate::core::challenge::{Condition, Outcome};
use crate::editor;
use crate::icon;
use crate::playback::{self, Playback};
use crate::sim;
use crate::storage;
use crate::theme;
use crate::widget::split::{Axis, Split};

/// The splash's full-bleed container; measuring it yields the viewport
/// size that seeds the game's divider.
const SPLASH: Id = Id::new("splash");

/// Top-level application state.
pub struct App {
    /// Light or dark chrome.
    mode: theme::Mode,
    screen: Screen,
}

/// Which screen is up.
enum Screen {
    /// The landing card: title, pitch, and the Continue button.
    Splash,
    /// The game proper. Boxed: it dwarfs the splash variant.
    Game(Box<Game>),
}

/// The game screen's state.
struct Game {
    playback: Playback,
    editor: editor::State,
    /// The last Apply's compile error, shown until the next Apply.
    /// Runtime errors live on [`Playback`]; `view` shows whichever is
    /// current.
    apply_error: Option<script::Error>,
    /// Picker entries, one per roster challenge, built once.
    choices: Vec<Choice>,
    /// Cached world geometry; cleared whenever the world changes.
    cache: canvas::Cache,
    /// The divider position of the workspace split, in pixels from the
    /// left; seeded from the measured viewport, `None` splits in half.
    divider: Option<Pixels>,
}

/// Resolves the active iced theme from the app's mode.
pub fn theme(app: &App) -> iced::Theme {
    app.mode.to_theme()
}

/// One challenge-picker entry: the roster index and its display label.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    index: usize,
    label: String,
}

/// Top-level application messages.
#[derive(Debug, Clone)]
pub enum Message {
    /// The splash's Continue button was pressed.
    Continue,
    /// The splash measurement came back; build the game with it.
    Measured(Option<selector::Target>),
    /// One playback frame is due. Deliberately a unit variant: the tick
    /// subscription's `Instant` is a different type on wasm, so it must
    /// never ride along.
    Tick,
    /// Start/Pause was pressed.
    Toggle,
    /// Restart was pressed (toolbar or failure banner).
    Restart,
    /// The success banner's next-challenge button was pressed.
    NextChallenge,
    /// A challenge was picked from the toolbar list.
    ChallengePicked(Choice),
    /// The timescale + button was pressed.
    SpeedUp,
    /// The timescale − button was pressed.
    SlowDown,
    /// The light/dark toggle was pressed.
    ToggleMode,
    /// Cmd+Page Up: step back one challenge.
    PreviousChallenge,
    /// The editor/world divider was dragged.
    SplitResized(Pixels),
    /// An editor pane message.
    Editor(editor::Message),
}

/// Boots onto the splash; the game (and the rhai engine underneath it)
/// is only built once Continue hands over a measured viewport.
pub fn boot() -> (App, Task<Message>) {
    (
        App {
            mode: theme::Mode::from_env(),
            screen: Screen::Splash,
        },
        Task::none(),
    )
}

/// Applies a message to the state. All simulation work happens here —
/// `view` only reads.
pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Continue => {
            return selector::find(SPLASH).map(Message::Measured);
        }
        Message::Measured(target) => {
            // Half the measured viewport seeds the divider so the twin
            // cards come up even; an unmeasurable splash (shouldn't
            // happen) falls back to the split's own halving.
            let divider = target
                .as_ref()
                .and_then(selector::Target::visible_bounds)
                .map(|bounds| Pixels(bounds.width / 2.0));
            app.screen = Screen::Game(Box::new(Game::new(divider)));
        }
        Message::ToggleMode => {
            app.mode = app.mode.toggle();
            // Cached geometry bakes the old palette in; force a redraw.
            if let Screen::Game(game) = &mut app.screen {
                game.cache.clear();
            }
        }
        message => {
            if let Screen::Game(game) = &mut app.screen {
                return game.update(message);
            }
        }
    }
    Task::none()
}

/// Ticks every 16 ms while playback runs (silent while paused, so a
/// paused app costs nothing), plus the challenge hotkeys:
/// Cmd+Page Down / Cmd+Page Up step through the roster. The splash
/// listens for nothing.
pub fn subscription(app: &App) -> Subscription<Message> {
    let Screen::Game(game) = &app.screen else {
        return Subscription::none();
    };
    let tick = if game.playback.is_running() {
        time::every(milliseconds(16)).map(|_| Message::Tick)
    } else {
        Subscription::none()
    };
    let hotkeys = keyboard::listen().filter_map(|event| match event {
        keyboard::Event::KeyPressed { key, modifiers, .. } if modifiers.command() => {
            match key.as_ref() {
                keyboard::Key::Named(Named::PageDown) => Some(Message::NextChallenge),
                keyboard::Key::Named(Named::PageUp) => Some(Message::PreviousChallenge),
                _ => None,
            }
        }
        _ => None,
    });
    Subscription::batch([tick, hotkeys])
}

/// Renders whichever screen is up.
pub fn view(app: &App) -> Element<'_, Message> {
    match &app.screen {
        Screen::Splash => splash(),
        Screen::Game(game) => game.view(app.mode),
    }
}

// --------------------------------------------------------------- splash

/// The landing card: title, pitch, credit, and Continue.
fn splash() -> Element<'static, Message> {
    let content = column![
        text("elevato").size(56).style(theme::text::primary),
        text("Program a bank of elevators in Rhai. Watch the simulation. Clear the challenges.")
            .size(16)
            .style(theme::text::secondary),
        button(
            row![icon::play().size(14), text("Continue").size(14)]
                .spacing(6)
                .align_y(Center),
        )
        .on_press(Message::Continue)
        .style(theme::button::primary),
        text("Based on Elevator Saga by Magnus Wolffelt")
            .size(12)
            .style(theme::text::secondary),
    ]
    .spacing(20)
    .align_x(Center);

    container(center(content))
        .id(SPLASH)
        .width(Fill)
        .height(Fill)
        .style(theme::container::root)
        .into()
}

// ----------------------------------------------------------------- game

impl Game {
    /// Builds the game: the last-saved code and timescale (else the
    /// built-in starter) on challenge 1, paused until Start is pressed
    /// (the original's flow), with the divider seeded from the measured
    /// viewport.
    fn new(divider: Option<Pixels>) -> Self {
        let (code, timescale) = match storage::load() {
            Some(saved) => (saved.code, saved.timescale),
            None => (playback::STARTER.to_string(), None),
        };
        // A saved program can be broken — Save persists raw text — so a
        // failed compile boots the starter engine underneath while the
        // editor keeps the saved text and shows its compile error.
        let (mut playback, apply_error) = match Playback::new(&code) {
            Ok(playback) => (playback, None),
            Err(error) => (
                Playback::new(playback::STARTER).expect("invariant: the built-in starter compiles"),
                Some(error),
            ),
        };
        if let Some(timescale) = timescale {
            playback.set_timescale(timescale);
        }
        let choices = playback
            .challenges()
            .iter()
            .enumerate()
            .map(|(index, challenge)| Choice {
                index,
                label: format!(
                    "Challenge #{} — {}",
                    index + 1,
                    describe(challenge.condition())
                ),
            })
            .collect();
        Game {
            playback,
            editor: editor::State::new(&code),
            apply_error,
            choices,
            cache: canvas::Cache::default(),
            divider,
        }
    }

    /// Applies a game-screen message.
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.playback.tick();
                self.cache.clear();
            }
            Message::Toggle => self.playback.toggle(),
            Message::Restart => {
                self.playback.restart();
                self.cache.clear();
            }
            Message::NextChallenge => {
                self.playback.next_challenge();
                self.cache.clear();
            }
            Message::ChallengePicked(choice) => {
                self.playback.select_challenge(choice.index);
                self.cache.clear();
            }
            Message::SpeedUp => self.playback.speed_up(),
            Message::SlowDown => self.playback.slow_down(),
            Message::PreviousChallenge => {
                self.playback.previous_challenge();
                self.cache.clear();
            }
            Message::SplitResized(position) => self.divider = Some(position),
            Message::Editor(message) => {
                let action = self.editor.update(message);
                if let Some(instruction) = action.instruction {
                    match instruction {
                        editor::Instruction::Apply(source) => {
                            self.apply_error = self.playback.apply(&source).err();
                            if self.apply_error.is_none() {
                                // Persistence is best-effort; a failed
                                // write just means nothing saved this
                                // time.
                                let _ = storage::save(&source, self.playback.timescale());
                                self.editor.mark_saved();
                            }
                            self.cache.clear();
                        }
                        editor::Instruction::Save(source) => {
                            let _ = storage::save(&source, self.playback.timescale());
                            self.editor.mark_saved();
                        }
                    }
                }
                return action.task.map(Message::Editor);
            }
            // Handled at the app level before routing here.
            Message::Continue | Message::Measured(_) | Message::ToggleMode => {}
        }
        Task::none()
    }

    /// Renders the game screen.
    fn view(&self, mode: theme::Mode) -> Element<'_, Message> {
        // Code on the left, world on the right — the reading order of
        // the game loop: write, then watch — twin rounded cards around
        // a draggable hairline divider.
        let workspace = container(
            Split::new(
                card(self.editor_pane()),
                card(self.world_pane()),
                self.divider,
                Axis::Vertical,
            )
            .on_resize(Message::SplitResized)
            .min_size_first(260)
            .min_size_second(320)
            .style(theme::split::divider),
        )
        .width(Fill)
        .height(Fill)
        .padding(8);

        container(column![self.toolbar(mode), self.stats_bar(), workspace])
            .width(Fill)
            .height(Fill)
            .style(theme::container::root)
            .into()
    }

    /// The world side of the split: the sim canvas, with the end-of-run
    /// banner stacked over it once the challenge decides.
    fn world_pane(&self) -> Element<'_, Message> {
        let world = canvas(sim::View::new(&self.playback, &self.cache))
            .width(Fill)
            .height(Fill);
        if self.playback.ended() {
            stack![world, center(banner(&self.playback))].into()
        } else {
            world.into()
        }
    }

    /// The editor side of the split, fed whichever script error is
    /// current: the last Apply's compile error until the next Apply,
    /// else whatever stopped the running attempt (an `init` failure or
    /// runtime throw).
    fn editor_pane(&self) -> Element<'_, Message> {
        let error = self.apply_error.as_ref().or_else(|| self.playback.error());
        self.editor.view(error).map(Message::Editor)
    }

    fn toolbar(&self, mode: theme::Mode) -> Element<'_, Message> {
        let picker = pick_list(
            self.choices.get(self.playback.challenge_index()).cloned(),
            self.choices.as_slice(),
            |choice: &Choice| choice.label.clone(),
        )
        .on_select(Message::ChallengePicked)
        .text_size(13);

        let toggle = if self.playback.is_running() {
            row![icon::pause().size(13), text("Pause").size(13)]
        } else {
            row![icon::play().size(13), text("Start").size(13)]
        }
        .spacing(6)
        .align_y(Center);

        let mode = match mode {
            theme::Mode::Dark => icon::sun(),
            theme::Mode::Light => icon::moon(),
        };

        // The main action anchors the row's end: …, Restart, then
        // Start.
        let bar = row![
            picker,
            space::horizontal(),
            button(icon::minus().size(13))
                .on_press(Message::SlowDown)
                .style(theme::button::ghost),
            text(format!("{}×", self.playback.timescale()))
                .size(14)
                .style(theme::text::primary),
            button(icon::plus().size(13))
                .on_press(Message::SpeedUp)
                .style(theme::button::ghost),
            button(mode.size(13))
                .on_press(Message::ToggleMode)
                .style(theme::button::ghost),
            button(
                row![icon::rotate_ccw().size(13), text("Restart").size(13)]
                    .spacing(6)
                    .align_y(Center)
            )
            .on_press(Message::Restart)
            .style(theme::button::outline),
            button(toggle)
                .on_press(Message::Toggle)
                .style(theme::button::primary),
        ]
        .spacing(8)
        .align_y(Center);

        container(bar)
            .width(Fill)
            .padding([8, 12])
            .style(theme::container::panel)
            .into()
    }

    fn stats_bar(&self) -> Element<'_, Message> {
        let stats = self.playback.stats();
        let bar = row![
            stat("Transported", stats.transported().to_string()),
            stat("Elapsed time", format!("{:.1}s", stats.elapsed())),
            stat(
                "Transported/s",
                format!("{:.2}", stats.transported_per_sec())
            ),
            stat("Avg waiting time", format!("{:.1}s", stats.avg_wait_time())),
            stat("Max waiting time", format!("{:.1}s", stats.max_wait_time())),
            stat("Moves", stats.move_count().to_string()),
            stat("Seed", self.playback.seed().to_string()),
        ]
        .spacing(24)
        .align_y(Center);

        container(bar)
            .width(Fill)
            .padding([6, 12])
            .style(theme::container::panel)
            .into()
    }
}

/// One workspace card: the identical rounded surface both sides of
/// the split live on.
fn card(content: Element<'_, Message>) -> Element<'_, Message> {
    container(content)
        .width(Fill)
        .height(Fill)
        .padding(8)
        .style(theme::container::pane)
        .into()
}

fn stat(label: &str, value: String) -> Element<'_, Message> {
    column![
        text(label).size(11).style(theme::text::secondary),
        text(value).size(14).style(theme::text::primary),
    ]
    .spacing(2)
    .into()
}

// --------------------------------------------------------------- banner

fn banner(playback: &Playback) -> Element<'_, Message> {
    let success = playback.outcome() == Outcome::Succeeded;

    let mut content = column![].spacing(12).align_x(Center);
    if success {
        content = content.push(text("Success!").size(24).style(theme::text::outcome(true)));
        if playback.challenge_index() + 1 < playback.challenges().len() {
            content = content.push(
                button(
                    row![
                        icon::skip_forward().size(14),
                        text("Next challenge").size(14)
                    ]
                    .spacing(6)
                    .align_y(Center),
                )
                .on_press(Message::NextChallenge)
                .style(theme::button::primary),
            );
        }
    } else {
        content = content.push(
            text("Challenge failed — maybe your program needs an improvement?")
                .size(18)
                .style(theme::text::outcome(false)),
        );
        content = content.push(
            button(
                row![icon::rotate_ccw().size(14), text("Restart").size(14)]
                    .spacing(6)
                    .align_y(Center),
            )
            .on_press(Message::Restart)
            .style(theme::button::outline),
        );
    }

    container(content)
        .padding(24)
        .style(theme::container::banner)
        .into()
}

// --------------------------------------------------------------- shared

/// The original's condition descriptions, verbatim (`challenges.js`
/// condition templates).
fn describe(condition: Condition) -> String {
    match condition {
        Condition::UserCountWithinTime {
            user_count,
            time_limit,
        } => format!("Transport {user_count} people in {time_limit} seconds or less"),
        Condition::UserCountWithMaxWaitTime {
            user_count,
            max_wait_time,
        } => format!(
            "Transport {user_count} people and let no one wait more than {max_wait_time} seconds"
        ),
        Condition::Both {
            user_count,
            time_limit,
            max_wait_time,
        } => format!(
            "Transport {user_count} people in {time_limit} seconds or less and let no one wait more than {max_wait_time} seconds"
        ),
        Condition::UserCountWithinMoves {
            user_count,
            move_limit,
        } => format!("Transport {user_count} people using {move_limit} elevator moves or less"),
        Condition::Demo => "Perpetual demo".to_string(),
    }
}
