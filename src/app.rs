//! The application: a splash screen that hands off - via a measured
//! viewport width - to the game screen: challenge toolbar, stats bar,
//! the program's view of the building, and the editor pane, a thin TEA
//! shell over
//! [`crate::playback::Playback`] composed with the [`crate::editor`]
//! cell.

use iced::event::{self, Event};
use iced::keyboard::{self, key::Named};
use iced::mouse;
use iced::time::{self, milliseconds};
use iced::widget::{
    Id, button, center, column, container, mouse_area, pick_list, row, selector, space, stack,
    text, text_editor,
};
use iced::padding;
use iced::window;
use iced::{Center, Element, Fill, Length, Pixels, Subscription, Task};

use elevato_api::Building;

use crate::core::challenge::{Condition, Outcome};
use crate::docs;
use crate::editor;
use crate::icon;
use crate::playback::{self, Playback, Program};
use crate::reload;
use crate::storage;
use crate::theme;
use crate::widget::balance::balance;
use crate::widget::pointer::pointer;
use crate::widget::split::{Axis, Split};

/// The splash's full-bleed container; measuring it yields the viewport
/// size that seeds the game's divider.
const SPLASH: Id = Id::new("splash");

/// Top-level application state.
pub struct App {
    /// Light or dark chrome.
    mode: theme::Mode,
    /// The player's program, booted afresh for every attempt.
    program: Program,
    /// Whether rawr runs the program, and so reports each save's reload.
    host: reload::Host,
    /// A save refused while the splash was up, for the game's strip.
    failed: Option<reload::Failure>,
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
    /// Who applies a save: the game hands it each one.
    host: reload::Host,
    playback: Playback,
    editor: editor::State,
    /// Picker entries, one per roster challenge, built once.
    choices: Vec<Choice>,
    /// The divider position of the workspace split, in pixels from the
    /// left; seeded from the measured viewport, `None` splits in half.
    divider: Option<Pixels>,
    /// Which face the narrow layout's single card shows.
    tab: Tab,
    /// Whether the code card shows a page of the library rather than
    /// the player's `main.rs`.
    reading: bool,
    /// The library's pages, read-only.
    docs: docs::State,
    /// Whether the wide layout shows the file sidebar.
    sidebar: bool,
    /// Whether the narrow layout's file drawer is open.
    drawer: bool,
    /// Live keyboard modifiers, tracked so clicks can tell whether the
    /// command key is down (cmd+click = jump to documentation).
    modifiers: keyboard::Modifiers,
    /// The latest known viewport width, deciding wide vs narrow layout.
    viewport_width: f32,
}

/// Below this viewport width the split gives way to the single-card
/// narrow layout (its two panes need 260 + 320 px just to exist).
const NARROW: f32 = 700.0;

/// The faces of the narrow layout's single card. The wide layout shows
/// both at once, the code card beside the world.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    /// The running world.
    Player,
    /// The code card: `main.rs` and the library.
    Code,
}

impl Tab {
    /// The face selector's label.
    fn label(self) -> &'static str {
        match self {
            Self::Player => "Simulation",
            Self::Code => "Code",
        }
    }
}

/// The library's pages in the file tree, sorted by file name.
const LIBRARY: [docs::Page; 4] = [
    docs::Page::Building,
    docs::Page::Elevator,
    docs::Page::Floor,
    docs::Page::Lib,
];

/// The width of the wide layout's file sidebar.
const SIDEBAR: f32 = 150.0;

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
    /// The splash measurement came back; build the game with it,
    /// resuming the game rawr restarted for an edit when `restarted`
    /// gives rawr's reason.
    Measured {
        target: Option<selector::Target>,
        restarted: Option<String>,
    },
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
    /// A face of the narrow layout's card was selected.
    Tab(Tab),
    /// The file tree's `main.rs`: back to the player's program.
    Write,
    /// A library page of the file tree.
    Read(docs::Page),
    /// The code card's header button: show or hide the file sidebar on
    /// the wide layout, open or close the drawer on the narrow one.
    ToggleFiles,
    /// Back through the reference's jump history (the tab-bar arrow, a
    /// keyboard's browser-back key, a mouse's back button, or the
    /// cmd+O / cmd+[ chords).
    DocsBack,
    /// Forward again (arrow, browser-forward key, mouse forward
    /// button, or the cmd+I / cmd+] chords).
    DocsForward,
    /// The tab-bar chip: jump to the selected name's definition - the
    /// touch answer to cmd+click (a double tap selects the word, the
    /// chip does the jump).
    DocsJump(docs::Page, usize),
    /// An API-reference pane message.
    Docs(docs::Message),
    /// The keyboard modifier state changed.
    ModifiersChanged(keyboard::Modifiers),
    /// The window was resized.
    Resized(f32),
    /// An editor pane message.
    Editor(editor::Message),
    /// A message the program's view produced, for the program.
    Program(playback::Input),
    /// rawr reported what it did with a save of the program.
    Reloaded(reload::Outcome),
}

/// Opens the game with the player's program running the elevators:
/// the program's `new`, `update`, `view` and `subscription`, as
/// [`iced::application`] takes an application's. Every attempt boots the
/// program afresh on the challenge's building, and the game hosts it as
/// its child, its view where the building shows.
pub fn run<P, M, V>(
    new: impl Fn(usize) -> (P, Task<M>) + 'static,
    update: impl Fn(&mut P, M, &Building) -> Task<M> + Clone + 'static,
    view: V,
    subscription: impl Fn(&P) -> Subscription<M> + Clone + 'static,
) -> iced::Result
where
    P: 'static,
    M: Send + 'static,
    V: for<'a> Fn(&'a P, &'a Building) -> Element<'a, M> + Clone + 'static,
{
    #[cfg(target_arch = "wasm32")]
    {
        console_log::init().expect("initialize logger");
        // The page's own hook (rawr's, which keeps the message for the
        // page) still runs after the console's.
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            console_error_panic_hook::hook(info);
            previous(info);
        }));
    }

    let settings = iced::Settings {
        // Safari now ships WebGPU, so wgpu auto-detects it on iOS - and
        // renders black. WebGL2 is the proven web path; force it on
        // wasm (Chrome included: consistency beats novelty here).
        #[cfg(target_arch = "wasm32")]
        backend: iced::Backend::Hardware(iced::backend::Api::OpenGL),
        ..iced::Settings::default()
    };

    let program = Program::new(new, update, view, subscription);
    iced::application(move || boot(program.clone()), self::update, self::view)
        .settings(settings)
        .subscription(self::subscription)
        .theme(theme)
        .font(theme::MONO_BYTES)
        .font(icon::FONT)
        .default_font(theme::MONO)
        .title("elevato.rs")
        .run()
}

/// Boots onto the splash; the game (and the player's program underneath
/// it) is only built once Continue hands over a measured viewport.
pub fn boot(program: Program) -> (App, Task<Message>) {
    (
        App {
            mode: theme::Mode::from_env(),
            program,
            host: reload::host(),
            failed: None,
            screen: Screen::Splash,
        },
        Task::none(),
    )
}

/// Applies a message to the state. All simulation work happens here -
/// `view` only reads.
pub fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::Continue => {
            return selector::find(SPLASH).map(|target| Message::Measured {
                target,
                restarted: None,
            });
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
        Message::Measured { target, restarted } => {
            // Half the measured viewport seeds the divider so the twin
            // cards come up even. Without one (a game the web page
            // restarts gets none) the split halves the latest viewport.
            let width = target
                .as_ref()
                .and_then(selector::Target::visible_bounds)
                .map(|bounds| bounds.width);
            let mut game = Box::new(Game::new(app.program.clone(), app.host, width));
            let mut task = match restarted {
                Some(reason) => game.update(Message::Reloaded(reload::Outcome::Restarted(reason))),
                None => Task::none(),
            };
            if let Some(failure) = app.failed.take() {
                task = Task::batch([
                    task,
                    game.update(Message::Reloaded(reload::Outcome::Failed(failure))),
                ]);
            }
            app.screen = Screen::Game(game);
            return task;
        }
        Message::Reloaded(outcome) => match (&mut app.screen, outcome) {
            // A game rawr restarted for an edit goes straight back to
            // the game, not the splash, once the splash is measured.
            (Screen::Splash, reload::Outcome::Restarted(reason)) => {
                return selector::find(SPLASH).map(move |target| Message::Measured {
                    target,
                    restarted: Some(reason.clone()),
                });
            }
            (Screen::Splash, reload::Outcome::Failed(failure)) => app.failed = Some(failure),
            (Screen::Splash, _) => {}
            (Screen::Game(game), outcome) => return game.update(Message::Reloaded(outcome)),
        },
        Message::ToggleMode => app.mode = app.mode.toggle(),
        message => {
            if let Screen::Game(game) = &mut app.screen {
                return game.update(message);
            }
        }
    }
    Task::none()
}

/// rawr's reload outcomes on every screen; on the game, a tick every
/// 16 ms while playback runs (silent while paused, so a paused app
/// costs nothing), plus the challenge hotkeys: Cmd+Page Down /
/// Cmd+Page Up step through the roster.
pub fn subscription(app: &App) -> Subscription<Message> {
    let reloads = app.host.outcomes().map(Message::Reloaded);
    let Screen::Game(game) = &app.screen else {
        return reloads;
    };
    let tick = if game.playback.is_running() {
        time::every(milliseconds(16)).map(|_| Message::Tick)
    } else {
        Subscription::none()
    };
    let hotkeys = keyboard::listen().filter_map(|event| match event {
        keyboard::Event::KeyPressed { key, modifiers, .. }
            if modifiers.command() || modifiers.control() =>
        {
            match key.as_ref() {
                keyboard::Key::Named(Named::PageDown) if modifiers.command() => {
                    Some(Message::NextChallenge)
                }
                keyboard::Key::Named(Named::PageUp) if modifiers.command() => {
                    Some(Message::PreviousChallenge)
                }
                // Vim's jump list is ctrl+O/ctrl+I on every OS; the
                // bracket pair follows the platform command key
                // (JetBrains/Xcode dialect).
                keyboard::Key::Character("o") if modifiers.control() => Some(Message::DocsBack),
                keyboard::Key::Character("i") if modifiers.control() => Some(Message::DocsForward),
                keyboard::Key::Character("[") if modifiers.command() => Some(Message::DocsBack),
                keyboard::Key::Character("]") if modifiers.command() => Some(Message::DocsForward),
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
    Subscription::batch([reloads, tick, hotkeys, resizes])
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
        balance(
            text("Program a bank of elevators in Rust. Hot-reload your code mid-run. Clear the challenges.")
                .size(16)
                .align_x(Center)
                .style(theme::text::secondary),
        ),
        button(
            row![icon::play().size(14), text("Play").size(14)]
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
    /// Builds the game: the player's program on challenge 1, paused
    /// until Start is pressed (the original's flow), its last-saved text
    /// in the editor (else the built-in starter) and its last timescale,
    /// with the divider seeded from the measured viewport.
    fn new(program: Program, host: reload::Host, viewport_width: Option<f32>) -> Self {
        let (code, timescale) = match storage::load() {
            Some(saved) => (saved.code, saved.timescale),
            None => (playback::STARTER.to_string(), None),
        };
        let mut playback = Playback::new(program);
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
                    "Challenge #{} - {}",
                    index + 1,
                    describe(challenge.condition())
                ),
            })
            .collect();
        Game {
            host,
            playback,
            editor: editor::State::new(&code),
            choices,
            divider: viewport_width.map(|width| Pixels(width / 2.0)),
            tab: Tab::Player,
            reading: false,
            docs: docs::State::new(),
            sidebar: true,
            drawer: false,
            modifiers: keyboard::Modifiers::default(),
            viewport_width: viewport_width.unwrap_or(1280.0),
        }
    }

    /// Applies a game-screen message.
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Tick => {
                self.playback.tick();
            }
            Message::Toggle => self.playback.toggle(),
            Message::Restart => {
                self.playback.restart();
            }
            Message::NextChallenge => {
                self.playback.next_challenge();
            }
            Message::ChallengePicked(choice) => {
                self.playback.select_challenge(choice.index);
            }
            Message::SpeedUp => self.playback.speed_up(),
            Message::SlowDown => self.playback.slow_down(),
            Message::PreviousChallenge => {
                self.playback.previous_challenge();
            }
            Message::Program(input) => self.playback.send(input),
            Message::SplitResized(position) => self.divider = Some(position),
            Message::Tab(tab) => self.tab = tab,
            Message::Write => {
                self.reading = false;
                self.drawer = false;
            }
            Message::Read(page) => {
                if self.docs.page() != page {
                    self.docs.open(page, 0);
                }
                self.reading = true;
                self.drawer = false;
            }
            Message::ToggleFiles => {
                if self.viewport_width < NARROW {
                    self.drawer = !self.drawer;
                } else {
                    self.sidebar = !self.sidebar;
                }
            }
            Message::DocsBack => {
                if self.reading {
                    self.docs.back();
                }
            }
            Message::DocsForward => {
                if self.reading {
                    self.docs.forward();
                }
            }
            Message::DocsJump(page, line) => {
                self.docs.open(page, line);
                self.reading = true;
                self.tab = Tab::Code;
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
                if jump
                    && let Some((page, line)) = self
                        .docs
                        .identifier_at_cursor()
                        .as_deref()
                        .and_then(|name| docs::resolve(name, Some(self.docs.page())))
                {
                    self.docs.open(page, line);
                }
                return action.task.map(Message::Docs);
            }
            Message::Editor(message) => {
                // A cmd+click in the editor opens the clicked name's
                // page of the library in the code card.
                let jump = self.modifiers.command()
                    && matches!(
                        &message,
                        editor::Message::Edited(text_editor::Action::Click(_))
                    );
                let action = self.editor.update(message);
                if jump
                    && let Some((page, line)) = self
                        .editor
                        .identifier_at_cursor()
                        .as_deref()
                        .and_then(|name| docs::resolve(name, None))
                {
                    self.docs.open(page, line);
                    self.reading = true;
                }
                if let Some(instruction) = action.instruction {
                    let (source, start) = match instruction {
                        editor::Instruction::Save(source) => (Some(source), false),
                        editor::Instruction::Run(source) => (source, true),
                    };
                    if let Some(source) = source {
                        let written = storage::save(
                            &source,
                            self.playback.timescale(),
                            self.playback.challenge_index(),
                        );
                        if written.is_ok() {
                            self.host.apply(&source);
                        }
                        self.editor.saved(written);
                    }
                    if start && !self.playback.is_running() {
                        self.playback.toggle();
                    }
                }
                return action.task.map(Message::Editor);
            }
            Message::Reloaded(outcome) => {
                match &outcome {
                    reload::Outcome::Swapped => self.playback.edit(),
                    // rawr restarted the process with the new code: resume
                    // the challenge the edit was applied on, running.
                    reload::Outcome::Restarted(_) => {
                        if let Some(challenge) = storage::load().and_then(|saved| saved.challenge) {
                            self.playback.select_challenge(challenge);
                        }
                        if !self.playback.is_running() {
                            self.playback.toggle();
                        }
                    }
                    reload::Outcome::Queued
                    | reload::Outcome::Restarting(_)
                    | reload::Outcome::Failed(_) => {}
                }
                self.editor.reloaded(outcome);
            }
            // Handled at the app level before routing here.
            Message::Continue
            | Message::Measured { .. }
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
        // Code on the left, world on the right - the reading order of
        // the game loop: write, then watch - twin rounded cards around
        // a draggable hairline divider.
        let workspace = container(
            Split::new(
                card(self.code_face()),
                card(self.player_face()),
                Some(self.divider.unwrap_or(Pixels(self.viewport_width / 2.0))),
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

    /// The identifier a double click (or double tap) left selected in
    /// the code card's file, resolved to its definition.
    fn selected_reference(&self) -> Option<(String, docs::Page, usize)> {
        if self.viewport_width < NARROW && self.tab != Tab::Code {
            return None;
        }
        let (name, preference) = if self.reading {
            (self.docs.selected_identifier()?, Some(self.docs.page()))
        } else {
            (self.editor.selected_identifier()?, None)
        };
        let (page, line) = docs::resolve(&name, preference)?;
        Some((name, page, line))
    }

    /// The code card: a header naming the open file over the file, with
    /// the file tree beside it - a collapsible sidebar on the wide
    /// layout, a drawer over the file on the narrow one.
    fn code_face(&self) -> Element<'_, Message> {
        let file = if self.reading {
            self.docs_face()
        } else {
            self.editor_pane()
        };
        let body: Element<'_, Message> = if self.viewport_width < NARROW {
            if self.drawer {
                // Tapping beside the drawer closes it.
                let drawer = container(self.file_tree())
                    .width(200)
                    .height(Fill)
                    .padding(8)
                    .style(theme::container::panel);
                stack![
                    file,
                    row![
                        drawer,
                        mouse_area(space().width(Fill).height(Fill))
                            .on_press(Message::ToggleFiles)
                    ]
                ]
                .into()
            } else {
                file
            }
        } else if self.sidebar {
            row![
                container(self.file_tree()).width(SIDEBAR).height(Fill),
                file
            ]
            .spacing(8)
            .into()
        } else {
            file
        };
        column![self.file_header(), body].spacing(6).into()
    }

    /// The open file's path - the sidebar's toggle beside it on the wide
    /// layout, the drawer's opener on the narrow one - and at its end
    /// the selection-jump chip and, while a page is read, the history
    /// arrows.
    fn file_header(&self) -> Element<'_, Message> {
        let path = if self.reading {
            format!("api/{}", self.docs.page().file_name())
        } else {
            "src/main.rs".to_string()
        };
        let mut name = row![
            text(path)
                .size(12)
                .font(theme::MONO)
                .style(theme::text::primary)
        ]
            .spacing(4)
            .align_y(Center);
        if self.reading {
            name = name.push(icon::lock().size(11).style(theme::text::secondary));
        }
        let mut header = if self.viewport_width < NARROW {
            row![
                button(name.push(icon::chevron_down().size(12)))
                    .padding([4, 6])
                    .on_press(Message::ToggleFiles)
                    .style(theme::button::ghost)
            ]
        } else {
            row![
                button(icon::panel_left().size(14))
                    .padding([4, 6])
                    .on_press(Message::ToggleFiles)
                    .style(theme::button::tab(self.sidebar)),
                container(name).padding([0, 4]),
            ]
        }
        .spacing(4)
        .align_y(Center);
        header = header.push(space::horizontal());
        if let Some((name, page, line)) = self.selected_reference() {
            header = header.push(
                button(
                    row![
                        text(name).size(12).font(theme::MONO),
                        icon::arrow_up_right().size(11)
                    ]
                    .spacing(3)
                    .align_y(Center),
                )
                .padding([4, 6])
                .on_press(Message::DocsJump(page, line))
                .style(theme::button::link),
            );
        }
        if self.reading {
            header = header.push(
                button(icon::arrow_left().size(13))
                    .padding([4, 6])
                    .on_press_maybe(self.docs.can_go_back().then_some(Message::DocsBack))
                    .style(theme::button::vanishing),
            );
            header = header.push(
                button(icon::arrow_right().size(13))
                    .padding([4, 6])
                    .on_press_maybe(self.docs.can_go_forward().then_some(Message::DocsForward))
                    .style(theme::button::vanishing),
            );
        }
        header.into()
    }

    /// The program's files: `src/main.rs`, the player's, and the
    /// library's read-only pages under `api/`.
    fn file_tree(&self) -> Element<'_, Message> {
        let folder = |name| {
            row![
                icon::folder().size(12).style(theme::text::secondary),
                text(name).size(12).font(theme::MONO).style(theme::text::secondary),
            ]
            .spacing(6)
            .align_y(Center)
        };
        let file = |name, active, message| {
            button(
                row![icon::file_code().size(12), text(name).size(12).font(theme::MONO)]
                    .spacing(6)
                    .align_y(Center),
            )
            .width(Fill)
            .padding([3, 6])
            .on_press(message)
            .style(theme::button::tab(active))
        };
        let mut tree = column![
            folder("src"),
            container(file("main.rs", !self.reading, Message::Write)).padding(padding::left(12)),
            space().height(4),
            folder("api"),
        ]
        .spacing(2);
        for page in LIBRARY {
            let active = self.reading && self.docs.page() == page;
            tree = tree.push(
                container(file(page.file_name(), active, Message::Read(page)))
                    .padding(padding::left(12)),
            );
        }
        tree.into()
    }

    /// The library page being read, with a pointer cursor while command is held
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

    /// The running world: the program's view of the building, with the
    /// end-of-run banner stacked over it once the challenge decides.
    fn player_face(&self) -> Element<'_, Message> {
        let world: Element<'_, Message> = match self.playback.view() {
            Some(view) => view.map(Message::Program),
            None => space().into(),
        };
        if self.playback.ended() {
            stack![world, center(banner(&self.playback))].into()
        } else {
            world
        }
    }

    /// The narrow (phone) layout: compact toolbar and stats over one
    /// card where the world and the code are tabs.
    fn narrow_view(&self, mode: theme::Mode) -> Element<'_, Message> {
        let face: Element<'_, Message> = match self.tab {
            Tab::Player => self.player_face(),
            Tab::Code => self.code_face(),
        };
        let tabs = row![
            space::horizontal(),
            tab_button(Tab::Player.label(), Tab::Player, self.tab),
            tab_button(Tab::Code.label(), Tab::Code, self.tab),
        ]
        .spacing(4);

        let workspace = container(
            column![
                tabs,
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
            stat("Seed", seed(&self.playback)),
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

    /// The editor side of the split, fed the program's panic when one
    /// stopped the running attempt.
    fn editor_pane(&self) -> Element<'_, Message> {
        pointer(
            self.editor.view(self.playback.error()).map(Message::Editor),
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
        .text_size(13)
        // The longest challenge labels are wider than a default window;
        // the picker gives way so the controls always fit.
        .width(Length::FillPortion(3).max(640));

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
            stat("Elapsed", format!("{:.1}s", stats.elapsed())),
            stat(
                "Transported/s",
                format!("{:.2}", stats.transported_per_sec())
            ),
            stat("Avg wait", format!("{:.1}s", stats.avg_wait_time())),
            stat("Max wait", format!("{:.1}s", stats.max_wait_time())),
            stat("Moves", stats.move_count().to_string()),
            stat("Seed", seed(&self.playback)),
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

/// One face selector of the narrow layout's tab bar.
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

/// The seed readout, marked once code changed under the attempt: the
/// seed alone no longer reproduces it, and Restart is the clean path.
fn seed(playback: &Playback) -> String {
    if playback.edited() {
        format!("{} (edited)", playback.seed())
    } else {
        playback.seed().to_string()
    }
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
            text("Challenge failed")
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
