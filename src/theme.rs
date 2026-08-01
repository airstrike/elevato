//! Semantic colors and widget styles. Every `.style()` call site in the
//! app passes a named fn from this module — no inline style closures —
//! and the sim canvas reads its colors from [`palette`]. Solid colors
//! only: gradients silently no-op on the wasm build.

use iced::{Color, Font, Theme, color};

/// The bundled monospace face for the editor and canvas labels: Fira
/// Code (OFL — `assets/fonts/OFL.txt`), embedded so both targets render
/// identically. The browser gives fontdb no system fonts, so wasm has
/// no `Font::MONOSPACE` to fall back on.
pub const MONO_BYTES: &[u8] = include_bytes!("../assets/fonts/FiraCode-Variable.ttf");

/// The bundled face by family name; the variable font's default
/// instance is Light, but the cosmic-text fork instances the `wght`
/// axis at the requested weight, so this renders Regular.
pub const MONO: Font = Font::new("Fira Code");

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
        Palette {
            text_primary: color!(0xe6e9f0),
            text_secondary: color!(0x9aa3b5),
            canvas_background: color!(0x1c1f26),
            panel: color!(0x262a33),
            floor_line: color!(0x3a4050),
            elevator_body: color!(0x3f7fc4),
            elevator_text: color!(0xffffff),
            indicator_lit: color!(0x7ee081),
            indicator_unlit: color!(0x4a5060),
            button_lit: color!(0xffd75e),
            passenger: color!(0xd8dce6),
            success: color!(0x6fbf73),
            failure: color!(0xe05a4e),
            syntax_comment: color!(0x6a7385),
            syntax_string: color!(0x98c379),
            syntax_number: color!(0xd19a66),
            syntax_constant: color!(0x56b6c2),
            syntax_operator: color!(0xa8b3c9),
            syntax_keyword: color!(0xc792ea),
        }
    } else {
        Palette {
            text_primary: color!(0x22262e),
            text_secondary: color!(0x5c6470),
            canvas_background: color!(0xf2f3f5),
            panel: color!(0xe6e8ec),
            floor_line: color!(0xc9cdd6),
            elevator_body: color!(0x3b76c0),
            elevator_text: color!(0xffffff),
            indicator_lit: color!(0x2e9e46),
            indicator_unlit: color!(0xb9bfcc),
            button_lit: color!(0xd9930d),
            passenger: color!(0x3a4150),
            success: color!(0x217a33),
            failure: color!(0xc23a2c),
            syntax_comment: color!(0x8a919e),
            syntax_string: color!(0x50a14f),
            syntax_number: color!(0x986801),
            syntax_constant: color!(0x0184bc),
            syntax_operator: color!(0x5c6470),
            syntax_keyword: color!(0xa626a4),
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

    /// The root backdrop behind everything.
    pub fn root(theme: &Theme) -> Style {
        let palette = palette(theme);
        Style {
            background: Some(palette.canvas_background.into()),
            text_color: Some(palette.text_primary),
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

    /// The Rhai code editor.
    pub fn code(theme: &Theme, status: Status) -> Style {
        let palette = palette(theme);
        let border_color = match status {
            Status::Focused { .. } => palette.elevator_body,
            Status::Active | Status::Hovered | Status::Disabled => palette.floor_line,
        };
        Style {
            background: palette.canvas_background.into(),
            border: Border {
                color: border_color,
                width: 1.0,
                radius: 4.0.into(),
            },
            placeholder: palette.text_secondary,
            value: palette.text_primary,
            selection: Color {
                a: 0.35,
                ..palette.elevator_body
            },
        }
    }
}

/// Preview: every semantic palette color as a labeled swatch, resolved
/// against the viewer's active theme at draw time.
#[cfg_attr(not(target_arch = "wasm32"), granita::preview)]
#[cfg(not(target_arch = "wasm32"))]
pub fn palette_swatches() -> iced::Element<'static, crate::app::Message> {
    use iced::widget::{column, container, row, space, text};

    let entries: [(&str, fn(&Palette) -> Color); 19] = [
        ("text_primary", |p| p.text_primary),
        ("text_secondary", |p| p.text_secondary),
        ("canvas_background", |p| p.canvas_background),
        ("panel", |p| p.panel),
        ("floor_line", |p| p.floor_line),
        ("elevator_body", |p| p.elevator_body),
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
#[cfg(not(target_arch = "wasm32"))]
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
