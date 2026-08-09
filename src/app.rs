//! The application: a splash screen that hands off — via a measured
//! viewport width — to the game screen: challenge toolbar, stats bar,
//! sim canvas, and the editor pane, a thin TEA shell over
//! [`crate::playback::Playback`] composed with the [`crate::editor`]
//! cell.

use iced::event::{self, Event};
use iced::keyboard::{self, key::Named};
use iced::mouse;
use iced::time::{self, milliseconds};
use iced::widget::{
    Id, button, canvas, center, column, container, pick_list, row, selector, space, stack, text,
    text_editor,
};
use iced::window;
use iced::{Center, Element, Fill, Pixels, Subscription, Task};

use crate::core::challenge::{Condition, Outcome};
use crate::docs;
use crate::editor;
use crate::icon;
use crate::playback::{self, Playback};
use crate::sim;
use crate::storage;
use crate::theme;
use crate::widget::pointer::pointer;
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
    /// Which face the right card shows: the world or the reference.
    tab: Tab,
    /// The API reference pane.
    docs: docs::State,
    /// Live keyboard modifiers, tracked so clicks can tell whether the
    /// command key is down (cmd+click = jump to documentation).
    modifiers: keyboard::Modifiers,
    /// The latest known viewport width, deciding wide vs narrow layout.
    viewport_width: f32,
}

/// Below this viewport width the split gives way to the single-card
/// narrow layout (its two panes need 260 + 320 px just to exist).
const NARROW: f32 = 700.0;

/// The card faces: two on the wide layout's right card, three on the
/// narrow layout's single card (where the editor is a tab too).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// The running world.
    Player,
    /// The code editor (narrow layout only).
    Code,
    /// The API reference.
    Api,
}

impl Tab {
    /// The face selector's label.
    fn label(self) -> &'static str {
        match self {
            Self::Player => "Player",
            Self::Code => "Code",
            Self::Api => "API",
        }
    }
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
    /// The splash's homage link: open the original game in a new tab.
    OpenOriginal,
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
    /// A tab of the right card was selected.
    Tab(Tab),
    /// Back through the reference's jump history (the tab-bar arrow, a
    /// keyboard's browser-back key, a mouse's back button, or the
    /// cmd+O / cmd+[ chords).
    DocsBack,
    /// Forward again (arrow, browser-forward key, mouse forward
    /// button, or the cmd+I / cmd+] chords).
    DocsForward,
    /// An API-reference pane message.
    Docs(docs::Message),
    /// The keyboard modifier state changed.
    ModifiersChanged(keyboard::Modifiers),
    /// The window was resized.
    Resized(f32),
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
        Message::OpenOriginal => {
            const ORIGINAL: &str = "https://play.elevatorsaga.com";
            #[cfg(target_arch = "wasm32")]
            if let Some(window) = web_sys::window() {
                let _ = window.open_with_url_and_target(ORIGINAL, "_blank");
            }
            #[cfg(not(target_arch = "wasm32"))]
            let _ = open::that(ORIGINAL);
        }
        Message::Measured(target) => {
            // Half the measured viewport seeds the divider so the twin
            // cards come up even; an unmeasurable splash (shouldn't
            // happen) falls back to the split's own halving.
            let width = target
                .as_ref()
                .and_then(selector::Target::visible_bounds)
                .map(|bounds| bounds.width);
            app.screen = Screen::Game(Box::new(Game::new(width)));
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
                // Editor-navigation dialects: vim's C-o/C-i and the
                // JetBrains/Xcode bracket pair.
                keyboard::Key::Character("o") | keyboard::Key::Character("[") => {
                    Some(Message::DocsBack)
                }
                keyboard::Key::Character("i") | keyboard::Key::Character("]") => {
                    Some(Message::DocsForward)
                }
                _ => None,
            }
        }
        keyboard::Event::ModifiersChanged(modifiers) => Some(Message::ModifiersChanged(modifiers)),
        _ => None,
    });
    let resizes = event::listen_with(|event, _status, _window| match event {
        Event::Window(window::Event::Resized(size)) => Some(Message::Resized(size.width)),
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::BrowserBack),
            ..
        }) => Some(Message::DocsBack),
        Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(Named::BrowserForward),
            ..
        }) => Some(Message::DocsForward),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Back)) => Some(Message::DocsBack),
        Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Forward)) => {
            Some(Message::DocsForward)
        }
        _ => None,
    });
    Subscription::batch([tick, hotkeys, resizes])
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
        text("elevato.rs").size(56).style(theme::text::primary),
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
        row![
            text("Based on ").size(12).style(theme::text::secondary),
            button(
                row![
                    text("Elevator Saga by Magnus Wolffelt").size(12),
                    icon::arrow_up_right().size(12),
                ]
                .spacing(4)
                .align_y(Center),
            )
            .padding(0)
            .on_press(Message::OpenOriginal)
            .style(theme::button::link),
        ]
        .align_y(Center),
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
    fn new(viewport_width: Option<f32>) -> Self {
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
            divider: viewport_width.map(|width| Pixels(width / 2.0)),
            tab: Tab::Player,
            docs: docs::State::new(),
            modifiers: keyboard::Modifiers::default(),
            viewport_width: viewport_width.unwrap_or(1280.0),
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
            Message::Tab(tab) => self.tab = tab,
            Message::DocsBack => {
                if self.tab == Tab::Api {
                    self.docs.back();
                }
            }
            Message::DocsForward => {
                if self.tab == Tab::Api {
                    self.docs.forward();
                }
            }
            Message::ModifiersChanged(modifiers) => self.modifiers = modifiers,
            Message::Resized(width) => self.viewport_width = width,
            Message::Docs(message) => {
                // A cmd+click in the reference keeps browsing: resolve
                // the identifier the click landed on and jump to it.
                let jump = self.modifiers.command()
                    && matches!(
                        &message,
                        docs::Message::Viewed(text_editor::Action::Click(_))
                    );
                let action = self.docs.update(message);
                if jump {
                    if let Some((page, line)) = self
                        .docs
                        .identifier_at_cursor()
                        .as_deref()
                        .and_then(|name| docs::resolve(name, Some(self.docs.page())))
                    {
                        self.docs.open(page, line);
                    }
                }
                return action.task.map(Message::Docs);
            }
            Message::Editor(message) => {
                // A cmd+click in the editor opens the clicked name's
                // documentation on the API tab.
                let jump = self.modifiers.command()
                    && matches!(
                        &message,
                        editor::Message::Edited(text_editor::Action::Click(_))
                    );
                let action = self.editor.update(message);
                if jump {
                    if let Some((page, line)) = self
                        .editor
                        .identifier_at_cursor()
                        .as_deref()
                        .and_then(|name| docs::resolve(name, None))
                    {
                        self.docs.open(page, line);
                        self.tab = Tab::Api;
                    }
                }
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
            Message::Continue
            | Message::Measured(_)
            | Message::ToggleMode
            | Message::OpenOriginal => {}
        }
        Task::none()
    }

    /// Renders the game screen: the split workspace on wide
    /// viewports, the single tabbed card on narrow ones (phones).
    fn view(&self, mode: theme::Mode) -> Element<'_, Message> {
        if self.viewport_width < NARROW {
            return self.narrow_view(mode);
        }
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

    /// The face of the right card: the world, or the API reference.
    fn world_pane(&self) -> Element<'_, Message> {
        match self.tab {
            // Code is a narrow-layout face; wide has the editor card.
            Tab::Player | Tab::Code => self.player_face(),
            Tab::Api => self.docs_face(),
        }
    }

    /// The face selectors, prefixed — when the reference is up and has
    /// somewhere to return to — by the back arrow.
    fn tab_bar(&self, tabs: &'static [Tab]) -> Element<'_, Message> {
        let mut bar = row![].spacing(4).align_y(Center);
        if self.tab == Tab::Api {
            bar = bar.push(
                button(icon::arrow_left().size(13))
                    .padding([4, 6])
                    .on_press_maybe(self.docs.can_go_back().then_some(Message::DocsBack))
                    .style(theme::button::ghost),
            );
            bar = bar.push(
                button(icon::arrow_right().size(13))
                    .padding([4, 6])
                    .on_press_maybe(self.docs.can_go_forward().then_some(Message::DocsForward))
                    .style(theme::button::ghost),
            );
        }
        for &tab in tabs {
            bar = bar.push(tab_button(tab.label(), tab, self.tab));
        }
        bar.into()
    }

    /// The API reference, with a pointer cursor while command is held
    /// (the cmd+click affordance).
    fn docs_face(&self) -> Element<'_, Message> {
        pointer(
            self.docs.view().map(Message::Docs),
            self.modifiers
                .command()
                .then_some(mouse::Interaction::Pointer),
        )
        .into()
    }

    /// The running world: the sim canvas with the end-of-run banner
    /// stacked over it once the challenge decides.
    fn player_face(&self) -> Element<'_, Message> {
        let world = canvas(sim::View::new(&self.playback, &self.cache))
            .width(Fill)
            .height(Fill);
        if self.playback.ended() {
            stack![world, center(banner(&self.playback))].into()
        } else {
            world.into()
        }
    }

    /// The narrow (phone) layout: compact toolbar and stats over one
    /// card where the world, the editor, and the reference are tabs.
    fn narrow_view(&self, mode: theme::Mode) -> Element<'_, Message> {
        let face: Element<'_, Message> = match self.tab {
            Tab::Player => self.player_face(),
            Tab::Code => self.editor_pane(),
            Tab::Api => self.docs_face(),
        };

        let workspace = container(
            column![
                row![
                    space::horizontal(),
                    self.tab_bar(&[Tab::Player, Tab::Code, Tab::Api])
                ],
                card(face)
            ]
            .spacing(6),
        )
        .width(Fill)
        .height(Fill)
        .padding(8);

        container(column![
            self.compact_toolbar(mode),
            self.compact_stats(),
            workspace
        ])
        .width(Fill)
        .height(Fill)
        .style(theme::container::root)
        .into()
    }

    /// The narrow toolbar: the picker on its own line, icon-only
    /// controls beneath it.
    fn compact_toolbar(&self, mode: theme::Mode) -> Element<'_, Message> {
        let picker = pick_list(
            self.choices.get(self.playback.challenge_index()).cloned(),
            self.choices.as_slice(),
            |choice: &Choice| choice.label.clone(),
        )
        .on_select(Message::ChallengePicked)
        .text_size(13)
        .width(Fill);

        let toggle = if self.playback.is_running() {
            icon::pause()
        } else {
            icon::play()
        };
        let mode = match mode {
            theme::Mode::Dark => icon::sun(),
            theme::Mode::Light => icon::moon(),
        };

        let controls = row![
            button(icon::minus().size(13))
                .on_press(Message::SlowDown)
                .style(theme::button::ghost),
            text(format!("{}×", self.playback.timescale()))
                .size(14)
                .style(theme::text::primary),
            button(icon::plus().size(13))
                .on_press(Message::SpeedUp)
                .style(theme::button::ghost),
            space::horizontal(),
            button(mode.size(13))
                .on_press(Message::ToggleMode)
                .style(theme::button::ghost),
            button(icon::rotate_ccw().size(13))
                .on_press(Message::Restart)
                .style(theme::button::outline),
            button(toggle.size(13))
                .on_press(Message::Toggle)
                .style(theme::button::primary),
        ]
        .spacing(8)
        .align_y(Center);

        container(column![picker, controls].spacing(8))
            .width(Fill)
            .padding([8, 12])
            .style(theme::container::panel)
            .into()
    }

    /// The narrow stats bar: the seven readouts in two rows.
    fn compact_stats(&self) -> Element<'_, Message> {
        let stats = self.playback.stats();
        let top = row![
            stat("Transported", stats.transported().to_string()),
            stat("Elapsed", format!("{:.1}s", stats.elapsed())),
            stat("Moves", stats.move_count().to_string()),
            stat("Seed", self.playback.seed().to_string()),
        ]
        .spacing(16);
        let bottom = row![
            stat(
                "Transported/s",
                format!("{:.2}", stats.transported_per_sec())
            ),
            stat("Avg wait", format!("{:.1}s", stats.avg_wait_time())),
            stat("Max wait", format!("{:.1}s", stats.max_wait_time())),
        ]
        .spacing(16);

        container(column![top, bottom].spacing(6))
            .width(Fill)
            .padding([6, 12])
            .style(theme::container::panel)
            .into()
    }

    /// The editor side of the split, fed whichever script error is
    /// current: the last Apply's compile error until the next Apply,
    /// else whatever stopped the running attempt (an `init` failure or
    /// runtime throw).
    fn editor_pane(&self) -> Element<'_, Message> {
        let error = self.apply_error.as_ref().or_else(|| self.playback.error());
        pointer(
            self.editor.view(error).map(Message::Editor),
            self.modifiers
                .command()
                .then_some(mouse::Interaction::Pointer),
        )
        .into()
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
            space::horizontal(),
            self.tab_bar(&[Tab::Player, Tab::Api]),
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

/// One face selector of the right card's tab bar.
fn tab_button(label: &str, tab: Tab, active: Tab) -> Element<'_, Message> {
    button(text(label).size(13))
        .on_press(Message::Tab(tab))
        .style(theme::button::tab(tab == active))
        .into()
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
