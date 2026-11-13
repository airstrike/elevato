//! elevato's color grade: the filter that turns a borrowed palette into
//! one that belongs on elevato's warm graphite and paper, with its
//! verdigris lamps. It works in OKLCH, so lightness - and with it a
//! color's contrast against the page - stays as the source palette set
//! it, and only the hue and colorfulness move:
//!
//! - the cool hues, teal through blue to violet, become elevato's
//!   greens ([`HUES`]): blues land on verdigris, violets on sea-teal,
//!   in their original order, while warm hues keep theirs,
//! - every color leans a little toward verdigris ([`TINT`]),
//! - loses some colorfulness ([`CHROMA`]), so nothing reads as neon,
//! - and warms by a touch of yellow ([`WARMTH`]), so cool slate greys
//!   sit on warm neutrals instead of fighting them.

use iced::Color;

/// elevato's verdigris lamp green (the dark theme's lit lamps): the hue
/// every graded color leans toward.
const VERDIGRIS: Color = Color::from_packed_rgb8(0x50b481);

/// The hue curve, as `(from, to)` degrees of OKLCH hue, linear between
/// points: the band from teal (175°) to violet (305°) folds into green
/// to sea-teal (140° to 190°; verdigris is 159°), and yellow-greens
/// make room for it.
const HUES: [(f32, f32); 5] = [
    (0.0, 0.0),
    (100.0, 100.0),
    (175.0, 140.0),
    (305.0, 190.0),
    (360.0, 360.0),
];

/// How far each color's hue and chroma move toward [`VERDIGRIS`].
const TINT: f32 = 0.05;

/// The share of colorfulness a graded color keeps.
const CHROMA: f32 = 0.85;

/// The yellow added to every color, in OKLab's `b`.
const WARMTH: f32 = 0.008;

/// `color` through elevato's grade.
pub fn grade(color: Color) -> Color {
    let (_, toward_a, toward_b) = oklab(VERDIGRIS);
    let (l, a, b) = oklab(color);
    let (a, b) = rehue(a, b);
    let a = (a + (toward_a - a) * TINT) * CHROMA;
    let b = (b + (toward_b - b) * TINT) * CHROMA + WARMTH;
    let mut graded = srgb(l, a, b);
    graded.a = color.a;
    graded
}

/// `(a, b)` with its hue moved along [`HUES`], its chroma kept.
fn rehue(a: f32, b: f32) -> (f32, f32) {
    let chroma = a.hypot(b);
    let hue = b.atan2(a).to_degrees().rem_euclid(360.0);
    let hue = HUES
        .windows(2)
        .find(|pair| hue <= pair[1].0)
        .map_or(hue, |pair| {
            let ((x0, y0), (x1, y1)) = (pair[0], pair[1]);
            y0 + (hue - x0) / (x1 - x0) * (y1 - y0)
        })
        .to_radians();
    (chroma * hue.cos(), chroma * hue.sin())
}

/// sRGB to OKLab's `(L, a, b)`.
fn oklab(color: Color) -> (f32, f32, f32) {
    let [r, g, b] = [color.r, color.g, color.b].map(linear);
    let l = (0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b).cbrt();
    let m = (0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b).cbrt();
    let s = (0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b).cbrt();
    (
        0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s,
        1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s,
        0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s,
    )
}

/// OKLab's `(L, a, b)` to sRGB, clamped into gamut.
fn srgb(l: f32, a: f32, b: f32) -> Color {
    let l_ = (l + 0.396_337_78 * a + 0.215_803_76 * b).powi(3);
    let m_ = (l - 0.105_561_346 * a - 0.063_854_17 * b).powi(3);
    let s_ = (l - 0.089_484_18 * a - 1.291_485_5 * b).powi(3);
    let r = 4.076_741_7 * l_ - 3.307_711_6 * m_ + 0.230_969_94 * s_;
    let g = -1.268_438 * l_ + 2.609_757_4 * m_ - 0.341_319_38 * s_;
    let b = -0.004_196_086_3 * l_ - 0.703_418_6 * m_ + 1.707_614_7 * s_;
    let [r, g, b] = [r, g, b].map(|channel| gamma(channel.clamp(0.0, 1.0)));
    Color::from_rgb(r, g, b)
}

/// An sRGB channel to linear light.
fn linear(channel: f32) -> f32 {
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// A linear-light channel to sRGB.
fn gamma(channel: f32) -> f32 {
    if channel <= 0.003_130_8 {
        channel * 12.92
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oklab_round_trips_srgb() {
        let color = Color::from_rgb8(0x7a, 0xa2, 0xf7);
        let (l, a, b) = oklab(color);
        let back = srgb(l, a, b);
        for (x, y) in [(color.r, back.r), (color.g, back.g), (color.b, back.b)] {
            assert!((x - y).abs() < 1e-3, "{x} != {y}");
        }
    }

    fn hue(color: Color) -> f32 {
        let (_, a, b) = oklab(color);
        b.atan2(a).to_degrees().rem_euclid(360.0)
    }

    #[test]
    fn blues_and_violets_become_elevato_greens() {
        // Tokyo City's function blue, type cyan and keyword violet.
        for hex in [0x7aa2f7, 0x0db9d7, 0xbb9af7] {
            let graded = hue(grade(Color::from_packed_rgb8(hex)));
            assert!((135.0..=195.0).contains(&graded), "{hex:06x} -> {graded}°");
        }
        // The warm hues stay warm: an orange is still an orange.
        let orange = hue(grade(Color::from_packed_rgb8(0xff9e64)));
        assert!((40.0..=70.0).contains(&orange), "orange -> {orange}°");
    }

    #[test]
    fn grading_keeps_lightness() {
        for hex in [0x7aa2f7, 0xbb9af7, 0xff9e64, 0x9ece6a, 0x526270, 0x34548a] {
            let color = Color::from_packed_rgb8(hex);
            let (before, ..) = oklab(color);
            let (after, ..) = oklab(grade(color));
            assert!(
                (before - after).abs() < 0.02,
                "{hex:06x}: {before} -> {after}"
            );
        }
    }
}
