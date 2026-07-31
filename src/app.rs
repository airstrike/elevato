//! The single application screen: challenge toolbar, stats bar, and the
//! sim canvas — a thin TEA shell over [`crate::playback::Playback`],
//! which owns every rule about running, seeding, and timescale.

use iced::time::{self, milliseconds};
use iced::widget::{button, canvas, center, column, container, pick_list, row, space, stack, text};
use iced::{Center, Element, Fill, Subscription, Task};

use crate::core::challenge::{Condition, Outcome};
use crate::playback::{self, Playback};
use crate::sim;
use crate::theme;

/// Top-level application state.
pub struct App {
    playback: Playback,
    /// Picker entries, one per roster challenge, built once at boot.
    choices: Vec<Choice>,
    /// Cached world geometry; cleared whenever the world changes.
    cache: canvas::Cache,
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
}

/// Builds the initial state and startup task: the built-in starter
/// program on challenge 1, paused until Start is pressed (the
/// original's flow).
pub fn boot() -> (App, Task<Message>) {
    let playback =
        Playback::new(playback::STARTER).expect("invariant: the built-in starter compiles");
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
    (
        App {
            playback,
            choices,
            cache: canvas::Cache::default(),
        },
        Task::none(),
    )
}

/// Applies a message to the state. All simulation work happens here —
/// `view` only reads.
pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Tick => {
            app.playback.tick();
            app.cache.clear();
        }
        Message::Toggle => app.playback.toggle(),
        Message::Restart => {
            app.playback.restart();
            app.cache.clear();
        }
        Message::NextChallenge => {
            app.playback.next_challenge();
            app.cache.clear();
        }
        Message::ChallengePicked(choice) => {
            app.playback.select_challenge(choice.index);
            app.cache.clear();
        }
        Message::SpeedUp => app.playback.speed_up(),
        Message::SlowDown => app.playback.slow_down(),
    }
    Task::none()
}

/// Ticks every 16 ms while playback runs; silent while paused, so a
/// paused app costs nothing.
pub fn subscription(app: &App) -> Subscription<Message> {
    if app.playback.is_running() {
        time::every(milliseconds(16)).map(|_| Message::Tick)
    } else {
        Subscription::none()
    }
}

/// Renders the screen.
pub fn view(app: &App) -> Element<'_, Message> {
    let world = canvas(sim::View::new(&app.playback, &app.cache))
        .width(Fill)
        .height(Fill);

    let world: Element<'_, Message> = if app.playback.ended() {
        stack![world, center(banner(&app.playback))].into()
    } else {
        world.into()
    };

    container(column![toolbar(app), stats_bar(app), world])
        .width(Fill)
        .height(Fill)
        .style(theme::container::root)
        .into()
}

// -------------------------------------------------------------- toolbar

fn toolbar(app: &App) -> Element<'_, Message> {
    let picker = pick_list(
        app.choices.get(app.playback.challenge_index()).cloned(),
        app.choices.as_slice(),
        |choice: &Choice| choice.label.clone(),
    )
    .on_select(Message::ChallengePicked)
    .text_size(13);

    let toggle = if app.playback.is_running() {
        "Pause"
    } else {
        "Start"
    };

    let bar = row![
        picker,
        button(text(toggle).size(13)).on_press(Message::Toggle),
        button(text("Restart").size(13)).on_press(Message::Restart),
        space::horizontal(),
        button(text("−").size(13)).on_press(Message::SlowDown),
        text(format!("{}×", app.playback.timescale()))
            .size(14)
            .style(theme::text::primary),
        button(text("+").size(13)).on_press(Message::SpeedUp),
    ]
    .spacing(8)
    .align_y(Center);

    container(bar)
        .width(Fill)
        .padding([8, 12])
        .style(theme::container::panel)
        .into()
}

// ------------------------------------------------------------ stats bar

fn stats_bar(app: &App) -> Element<'_, Message> {
    let stats = app.playback.stats();
    let mut bar = row![
        stat("Transported", stats.transported().to_string()),
        stat("Elapsed time", format!("{:.1}s", stats.elapsed())),
        stat(
            "Transported/s",
            format!("{:.2}", stats.transported_per_sec())
        ),
        stat("Avg waiting time", format!("{:.1}s", stats.avg_wait_time())),
        stat("Max waiting time", format!("{:.1}s", stats.max_wait_time())),
        stat("Moves", stats.move_count().to_string()),
        stat("Seed", app.playback.seed().to_string()),
    ]
    .spacing(24)
    .align_y(Center);

    if let Some(error) = app.playback.error() {
        bar = bar.push(text(error.to_string()).size(12).style(theme::text::failure));
    }

    container(bar)
        .width(Fill)
        .padding([6, 12])
        .style(theme::container::panel)
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
            content = content
                .push(button(text("Next challenge").size(14)).on_press(Message::NextChallenge));
        }
    } else {
        content = content.push(
            text("Challenge failed — maybe your program needs an improvement?")
                .size(18)
                .style(theme::text::outcome(false)),
        );
        content = content.push(button(text("Restart").size(14)).on_press(Message::Restart));
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
