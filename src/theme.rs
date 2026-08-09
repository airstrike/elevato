//! Semantic colors and widget styles. Every `.style()` call site in the
//! app passes a named fn from this module — no inline style closures —
//! and the sim canvas reads its colors from [`palette`]. Solid colors
//! only: gradients silently no-op on the wasm build.

use iced::theme::palette::Seed;
use iced::{Color, Font, Theme, color};

/// Light or dark, the app-level choice driving [`Mode::to_theme`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    Light,
    #[default]
    Dark,
}

impl Mode {
    /// Reads `ELEVATO_THEME` natively; on the web, the browser's
    /// `prefers-color-scheme`, so the app agrees with the landing card.
    pub fn from_env() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(value) = std::env::var("ELEVATO_THEME") {
            if value.eq_ignore_ascii_case("light") {
                return Self::Light;
            }
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(query) = web_sys::window()
            .and_then(|window| window.match_media("(prefers-color-scheme: light)").ok())
            .flatten()
        {
            if query.matches() {
                return Self::Light;
            }
        }
        Self::default()
    }

    /// The other mode.
    pub fn toggle(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Dark,
        }
    }

    /// Builds the iced theme from elevato's seed colors, so stock
    /// widget defaults (pick_list, sliders, text ladders) inherit the
    /// design instead of iced's stock palette. Surface colors beyond
    /// the seed are hand-picked in [`palette`].
    pub fn to_theme(self) -> Theme {
        let (name, seed) = match self {
            Self::Light => (
                "elevato",
                Seed {
                    background: color!(0xf2efe9),
                    text: color!(0x2b2a27),
                    primary: color!(0x2e8c5f),
                    success: color!(0x2e7d43),
                    warning: color!(0xb08a2e),
                    danger: color!(0xc0473a),
                },
            ),
            Self::Dark => (
                "elevato dark",
                Seed {
                    background: color!(0x191816),
                    text: color!(0xece7db),
                    primary: color!(0x50b481),
                    success: color!(0x5fbf77),
                    warning: color!(0xd9c26a),
                    danger: color!(0xd95f4e),
                },
            ),
        };
        Theme::custom(name.to_string(), seed)
    }
}

/// The one bundled face — Geist Mono (OFL, `assets/fonts/OFL.txt`) —
/// voicing the editor, chrome, and canvas labels alike. Embedded so
/// both targets render identically; the browser gives fontdb no system
/// fonts, so wasm has nothing to fall back on. The cosmic-text fork
/// instances the variable `wght` axis at the requested weight.
pub const MONO_BYTES: &[u8] = include_bytes!("../assets/fonts/GeistMono-Variable.ttf");

/// The bundled face by family name.
pub const MONO: Font = Font::new("Geist Mono");

/// Named semantic colors, resolved per theme brightness by [`palette`].
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    /// Headline and value text.
    pub text_primary: Color,
    /// Labels and captions.
    pub text_secondary: Color,
    /// The world canvas backdrop.
    pub canvas_background: Color,
    /// Toolbar, stats bar, and banner surfaces.
    pub panel: Color,
    /// Floor baselines and hairline borders.
    pub floor_line: Color,
    /// The elevator car body.
    pub elevator_body: Color,
    /// The cab's header band (lamps + floor readout live on it).
    pub elevator_band: Color,
    /// Text drawn on the elevator body.
    pub elevator_text: Color,
    /// A lit direction indicator or floor call button.
    pub indicator_lit: Color,
    /// An unlit indicator or call button.
    pub indicator_unlit: Color,
    /// A lit in-elevator destination button.
    pub button_lit: Color,
    /// Passenger figures.
    pub passenger: Color,
    /// Success green (banner text).
    pub success: Color,
    /// Failure red (banner text, script errors).
    pub failure: Color,
    /// Editor tokens: comments.
    pub syntax_comment: Color,
    /// Editor tokens: string literals.
    pub syntax_string: Color,
    /// Editor tokens: numeric literals.
    pub syntax_number: Color,
    /// Editor tokens: `true`/`false`.
    pub syntax_constant: Color,
    /// Editor tokens: operators and closure pipes.
    pub syntax_operator: Color,
    /// Editor tokens: keywords.
    pub syntax_keyword: Color,
}

/// Resolves the semantic palette for the current theme's brightness.
pub fn palette(theme: &Theme) -> Palette {
    if is_dark(theme) {
        // Machine room at night: warm graphite (no blue cast), ivory
        // text, verdigris lamp-green accents — old elevator-hall lamps.
        Palette {
            text_primary: color!(0xece7db),
            text_secondary: color!(0x98948a),
            canvas_background: color!(0x191816),
            panel: color!(0x22211e),
            floor_line: color!(0x35332e),
            elevator_body: color!(0x3b3a36),
            elevator_band: color!(0x2e2d2a),
            elevator_text: color!(0xf0ecdf),
            indicator_lit: color!(0x6fd692),
            indicator_unlit: color!(0x413f3a),
            button_lit: color!(0xf2eecf),
            passenger: color!(0xd4cfc2),
            success: color!(0x5fbf77),
            failure: color!(0xd95f4e),
            syntax_comment: color!(0x8a867c),
            syntax_string: color!(0x9ec978),
            syntax_number: color!(0xc9a45c),
            syntax_constant: color!(0x62c1c9),
            syntax_operator: color!(0xaaa69b),
            syntax_keyword: color!(0x50b481),
        }
    } else {
        // Daylight lobby: warm paper, ink text, the same lamp-green.
        Palette {
            text_primary: color!(0x2b2a27),
            text_secondary: color!(0x6f6c64),
            canvas_background: color!(0xf2efe9),
            panel: color!(0xe8e4da),
            floor_line: color!(0xcfcabd),
            elevator_body: color!(0x45443f),
            elevator_band: color!(0x393833),
            elevator_text: color!(0xf4f1e8),
            indicator_lit: color!(0x2e8c5f),
            indicator_unlit: color!(0xbfbaab),
            button_lit: color!(0x8a6d1f),
            passenger: color!(0x3f3d37),
            success: color!(0x2e7d43),
            failure: color!(0xc0473a),
            syntax_comment: color!(0x928d80),
            syntax_string: color!(0x4e8d3e),
            syntax_number: color!(0xa07617),
            syntax_constant: color!(0x137e8a),
            syntax_operator: color!(0x6f6c64),
            syntax_keyword: color!(0x2e8c5f),
        }
    }
}

fn is_dark(theme: &Theme) -> bool {
    let background = theme.palette().background.base.color;
    0.299 * background.r + 0.587 * background.g + 0.114 * background.b < 0.5
}

pub mod text {
    //! Text colors for `.style()` on `text` widgets.

    use iced::Theme;
    use iced::widget::text::Style;

    use super::palette;

    /// Headline and value text.
    pub fn primary(theme: &Theme) -> Style {
        Style {
            color: Some(palette(theme).text_primary),
        }
    }

    /// Labels and captions.
    pub fn secondary(theme: &Theme) -> Style {
        Style {
            color: Some(palette(theme).text_secondary),
        }
    }

    /// Script-error text.
    pub fn failure(theme: &Theme) -> Style {
        Style {
            color: Some(palette(theme).failure),
        }
    }

    /// Banner headline, green on success and red on failure.
    pub fn outcome(success: bool) -> impl Fn(&Theme) -> Style {
        move |theme| {
            let palette = palette(theme);
            Style {
                color: Some(if success {
                    palette.success
                } else {
                    palette.failure
                }),
            }
        }
    }
}

pub mod container {
    //! Surface styles for `.style()` on `container` widgets.

    use iced::widget::container::Style;
    use iced::{Theme, border};

    use super::palette;

    /// The root backdrop behind everything: the panel shade, so the
    /// workspace cards (drawn in the canvas shade) read as raised
    /// surfaces against it.
    pub fn root(theme: &Theme) -> Style {
        let palette = palette(theme);
        Style {
            background: Some(palette.panel.into()),
            text_color: Some(palette.text_primary),
            ..Style::default()
        }
    }

    /// A workspace card — the editor and the world get the *same*
    /// rounded surface, so the two halves of the split are twins.
    pub fn pane(theme: &Theme) -> Style {
        let palette = palette(theme);
        Style {
            background: Some(palette.canvas_background.into()),
            border: border::rounded(8.0).color(palette.floor_line).width(1),
            ..Style::default()
        }
    }

    /// Toolbar and stats-bar strips.
    pub fn panel(theme: &Theme) -> Style {
        Style {
            background: Some(palette(theme).panel.into()),
            ..Style::default()
        }
    }

    /// The end-of-challenge banner card floating over the canvas.
    pub fn banner(theme: &Theme) -> Style {
        let palette = palette(theme);
        Style {
            background: Some(palette.panel.into()),
            border: border::rounded(8.0).color(palette.floor_line).width(1),
            ..Style::default()
        }
    }

    /// The script-error strip under the editor.
    pub fn error_panel(theme: &Theme) -> Style {
        let palette = palette(theme);
        Style {
            background: Some(palette.panel.into()),
            border: border::rounded(4.0).color(palette.failure).width(1),
            ..Style::default()
        }
    }
}

pub mod text_editor {
    //! Styles for `.style()` on `text_editor` widgets.

    use iced::widget::text_editor::{Status, Style};
    use iced::{Border, Color, Theme};

    use super::palette;

    /// The Rhai code editor: chromeless — its card (the workspace pane
    /// it sits in) provides the surface and border, so the editor and
    /// the world canvas dress identically.
    pub fn code(theme: &Theme, status: Status) -> Style {
        let palette = palette(theme);
        let _ = status;
        Style {
            background: Color::TRANSPARENT.into(),
            border: Border::default(),
            placeholder: palette.text_secondary,
            value: palette.text_primary,
            selection: Color {
                a: 0.35,
                ..palette.elevator_body
            },
        }
    }
}

/// Granita previews — native-only dev tooling. A plain module `#[cfg]`
/// (not `cfg_attr` on the fn) because granita's source walker matches
/// the literal `#[granita::preview]` attribute path and cannot see
/// through `cfg_attr`.
#[cfg(not(target_arch = "wasm32"))]
pub mod previews {
    use super::*;

    /// Preview: every semantic palette color as a labeled swatch,
    /// resolved against the viewer's active theme at draw time.
    #[granita::preview]
    pub fn palette_swatches() -> iced::Element<'static, crate::app::Message> {
        use iced::widget::{column, container, row, space, text};

        let entries: [(&str, fn(&Palette) -> Color); 20] = [
            ("text_primary", |p| p.text_primary),
            ("text_secondary", |p| p.text_secondary),
            ("canvas_background", |p| p.canvas_background),
            ("panel", |p| p.panel),
            ("floor_line", |p| p.floor_line),
            ("elevator_body", |p| p.elevator_body),
            ("elevator_band", |p| p.elevator_band),
            ("elevator_text", |p| p.elevator_text),
            ("indicator_lit", |p| p.indicator_lit),
            ("indicator_unlit", |p| p.indicator_unlit),
            ("button_lit", |p| p.button_lit),
            ("passenger", |p| p.passenger),
            ("success", |p| p.success),
            ("failure", |p| p.failure),
            ("syntax_comment", |p| p.syntax_comment),
            ("syntax_string", |p| p.syntax_string),
            ("syntax_number", |p| p.syntax_number),
            ("syntax_constant", |p| p.syntax_constant),
            ("syntax_operator", |p| p.syntax_operator),
            ("syntax_keyword", |p| p.syntax_keyword),
        ];

        container(
            column(entries.into_iter().map(|(name, pick)| {
                row![
                    container(space::horizontal().width(56).height(22)).style(swatch(pick)),
                    text(name).font(MONO).size(13),
                ]
                .spacing(12)
                .align_y(iced::Center)
                .into()
            }))
            .spacing(6),
        )
        .padding(16)
        .into()
    }

    /// A swatch chip filled with one palette color, theme-resolved at draw.
    fn swatch(pick: fn(&Palette) -> Color) -> impl Fn(&Theme) -> iced::widget::container::Style {
        move |theme| iced::widget::container::Style {
            background: Some(pick(&palette(theme)).into()),
            border: iced::Border {
                radius: 4.0.into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
}

pub mod button {
    //! Button styles for `.style()` on `button` widgets: a deliberate
    //! hierarchy — one filled `primary` per surface, `outline` for
    //! secondary actions (shadcn-style), `ghost` for tertiary ones.

    use iced::widget::button::{Status, Style};
    use iced::{Border, Theme, border};

    use super::palette;

    /// Filled accent — the surface's main action (Start, Apply).
    pub fn primary(theme: &Theme, status: Status) -> Style {
        let ladder = theme.palette();
        let base = Style {
            background: Some(ladder.primary.base.color.into()),
            text_color: ladder.primary.base.text,
            border: border::rounded(6),
            ..Style::default()
        };
        match status {
            Status::Active | Status::Pressed => base,
            Status::Hovered => Style {
                background: Some(ladder.primary.strong.color.into()),
                ..base
            },
            Status::Disabled => faded(base),
        }
    }

    /// Outlined, quiet — secondary actions (Restart, Save).
    pub fn outline(theme: &Theme, status: Status) -> Style {
        let palette = palette(theme);
        let base = Style {
            background: None,
            text_color: palette.text_primary,
            border: Border {
                color: palette.floor_line,
                width: 1.0,
                radius: 6.0.into(),
            },
            ..Style::default()
        };
        match status {
            Status::Active | Status::Pressed => base,
            Status::Hovered => Style {
                background: Some(palette.panel.into()),
                border: Border {
                    color: palette.text_secondary,
                    ..base.border
                },
                ..base
            },
            Status::Disabled => faded(base),
        }
    }

    /// Bare — tertiary actions (speed steps, resets, mode toggle).
    pub fn ghost(theme: &Theme, status: Status) -> Style {
        let palette = palette(theme);
        let base = Style {
            background: None,
            text_color: palette.text_secondary,
            border: border::rounded(6),
            ..Style::default()
        };
        match status {
            Status::Active | Status::Pressed => base,
            Status::Hovered => Style {
                background: Some(palette.panel.into()),
                text_color: palette.text_primary,
                ..base
            },
            Status::Disabled => faded(base),
        }
    }

    /// A text link: bare accent text that brightens on hover — the
    /// splash's homage to the original.
    pub fn link(theme: &Theme, status: Status) -> Style {
        let ladder = theme.palette();
        let base = Style {
            background: None,
            text_color: ladder.primary.base.color,
            border: border::rounded(6),
            ..Style::default()
        };
        match status {
            Status::Active | Status::Pressed => base,
            Status::Hovered => Style {
                text_color: ladder.primary.strong.color,
                ..base
            },
            Status::Disabled => faded(base),
        }
    }

    /// A face selector on the right card's tab bar: quiet when
    /// inactive, raised on the panel shade when active.
    pub fn tab(active: bool) -> impl Fn(&Theme, Status) -> Style {
        move |theme, status| {
            let palette = palette(theme);
            let base = if active {
                Style {
                    background: Some(palette.panel.into()),
                    text_color: palette.text_primary,
                    border: border::rounded(6),
                    ..Style::default()
                }
            } else {
                Style {
                    background: None,
                    text_color: palette.text_secondary,
                    border: border::rounded(6),
                    ..Style::default()
                }
            };
            match status {
                Status::Active | Status::Pressed => base,
                Status::Hovered => Style {
                    text_color: palette.text_primary,
                    background: Some(palette.panel.into()),
                    ..base
                },
                Status::Disabled => faded(base),
            }
        }
    }

    fn faded(base: Style) -> Style {
        Style {
            text_color: base.text_color.scale_alpha(0.4),
            background: base
                .background
                .map(|background| background.scale_alpha(0.4)),
            border: Border {
                color: base.border.color.scale_alpha(0.4),
                ..base.border
            },
            ..base
        }
    }
}

pub mod split {
    //! Divider styling for the workspace [`Split`](crate::widget::split).

    use iced::Background;
    use iced::Theme;

    use crate::widget::split::{Status, Style};

    use super::palette;

    /// A hairline divider that warms to the accent while grabbed.
    pub fn divider(theme: &Theme, status: Status) -> Style {
        let palette = palette(theme);
        let base = Style {
            divider_background: Background::Color(palette.floor_line),
            ..Style::default()
        };
        match status {
            Status::Active | Status::Disabled => base,
            Status::Hovered | Status::Dragging => Style {
                divider_background: Background::Color(theme.palette().primary.base.color),
                ..base
            },
        }
    }
}
