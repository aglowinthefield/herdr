//! The `terminal` theme, adapted to the host terminal's reported colors.
//!
//! The built-in `terminal` palette uses named ANSI colors so it follows the
//! host's color scheme. Surfaces such as `DarkGray` only read well on dark
//! backgrounds, though: on a light scheme the active tab and selected rows
//! turn into low-contrast gray blocks. Once the host has answered the OSC 10
//! and 11 queries, this module derives the surfaces from its real foreground
//! and background instead, and keeps every text/background pair readable.

use ratatui::style::Color;

use super::state::Palette;
use crate::terminal_theme::{RgbColor, TerminalTheme};

type Rgb = (u8, u8, u8);

/// Minimum contrast for text drawn on the accent (focused tab, modal titles).
const ACCENT_TEXT_CONTRAST: f64 = 4.5;
/// Minimum contrast for state colors drawn on the terminal background.
const STATE_CONTRAST: f64 = 3.0;
/// Minimum contrast for muted text on the surfaces it sits on.
const MUTED_TEXT_CONTRAST: f64 = 3.0;

const ANSI_RED: usize = 1;
const ANSI_GREEN: usize = 2;
const ANSI_YELLOW: usize = 3;
const ANSI_BLUE: usize = 4;
const ANSI_CYAN: usize = 6;
const ANSI_BRIGHT_RED: usize = 9;

impl Palette {
    /// Derive the `terminal` palette from the host's colors. Returns `None`
    /// until the host has reported both its foreground and background, in
    /// which case callers keep the named-ANSI palette.
    pub fn terminal_adapted(host: &TerminalTheme) -> Option<Self> {
        let fg = rgb(host.foreground?);
        let bg = rgb(host.background?);
        let ansi = |index: usize| host.palette[index].map(rgb);
        let tint = |amount: f32| mix(bg, fg, amount);

        let surface0 = tint(0.07);
        let surface_dim = tint(0.10);
        let active_row_bg = tint(0.13);
        let surface1 = tint(0.16);
        let selection_bg = tint(0.22);
        // Text lands on every surface, so it has to clear the lightest and
        // darkest of them, not just the background.
        let surfaces = [
            bg,
            surface0,
            surface_dim,
            active_row_bg,
            surface1,
            selection_bg,
        ];
        let on_surfaces = |color: Rgb, minimum: f64| {
            surfaces
                .iter()
                .chain(surfaces.iter())
                .fold(color, |color, &surface| {
                    readable(color, surface, fg, minimum)
                })
        };
        let text = on_surfaces(fg, 4.5);

        let muted = |amount: f32| on_surfaces(tint(amount), MUTED_TEXT_CONTRAST);
        let overlay0 = muted(0.50);
        let subtext0 = muted(0.62);
        let overlay1 = muted(0.72);

        let state = |color: Option<Rgb>, fallback: Rgb| {
            readable(color.unwrap_or(fallback), bg, fg, STATE_CONTRAST)
        };
        // Without a reported palette, fall back to the foreground so the
        // accent still contrasts; a guessed hue could vanish into the background.
        let blue = state(ansi(ANSI_BLUE), fg);
        let accent = readable(blue, surface_dim, fg, ACCENT_TEXT_CONTRAST);
        let yellow = state(ansi(ANSI_YELLOW), fg);

        Some(Self {
            accent: color(accent),
            panel_bg: Color::Reset,
            sidebar_bg: Color::Reset,
            active_row_bg: color(active_row_bg),
            selection_bg: color(selection_bg),
            surface0: color(surface0),
            surface1: color(surface1),
            surface_dim: color(surface_dim),
            overlay0: color(overlay0),
            overlay1: color(overlay1),
            text: color(text),
            subtext0: color(subtext0),
            mauve: color(subtext0),
            green: color(state(ansi(ANSI_GREEN), fg)),
            yellow: color(yellow),
            red: color(state(ansi(ANSI_BRIGHT_RED).or(ansi(ANSI_RED)), fg)),
            blue: color(blue),
            teal: color(state(ansi(ANSI_CYAN), fg)),
            peach: color(yellow),
        })
    }
}

fn rgb(color: RgbColor) -> Rgb {
    (color.r, color.g, color.b)
}

fn color((r, g, b): Rgb) -> Color {
    Color::Rgb(r, g, b)
}

fn mix(base: Rgb, target: Rgb, amount: f32) -> Rgb {
    fn channel(base: u8, target: u8, amount: f32) -> u8 {
        (f32::from(base) + (f32::from(target) - f32::from(base)) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    }
    (
        channel(base.0, target.0, amount),
        channel(base.1, target.1, amount),
        channel(base.2, target.2, amount),
    )
}

/// Move `color` toward `fg` until it reaches `minimum` contrast against
/// `against`. If even `fg` falls short, keep going toward black or white.
fn readable(color: Rgb, against: Rgb, fg: Rgb, minimum: f64) -> Rgb {
    if contrast(color, against) >= minimum {
        return color;
    }
    let extreme = if contrast((255, 255, 255), against) >= contrast((0, 0, 0), against) {
        (255, 255, 255)
    } else {
        (0, 0, 0)
    };
    for target in [fg, extreme] {
        if contrast(target, against) < minimum && target != extreme {
            continue;
        }
        for step in 1..=20 {
            let candidate = mix(color, target, step as f32 / 20.0);
            if contrast(candidate, against) >= minimum {
                return candidate;
            }
        }
    }
    extreme
}

fn luminance((r, g, b): Rgb) -> f64 {
    fn channel(value: u8) -> f64 {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b)
}

fn contrast(a: Rgb, b: Rgb) -> f64 {
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(fg: Rgb, bg: Rgb, ansi: &[(usize, Rgb)]) -> TerminalTheme {
        let rgb = |(r, g, b): Rgb| RgbColor { r, g, b };
        let mut theme = TerminalTheme {
            foreground: Some(rgb(fg)),
            background: Some(rgb(bg)),
            ..TerminalTheme::default()
        };
        for &(index, color) in ansi {
            theme.palette[index] = Some(rgb(color));
        }
        theme
    }

    fn as_rgb(color: Color) -> Rgb {
        let Color::Rgb(r, g, b) = color else {
            panic!("expected an RGB color, got {color:?}");
        };
        (r, g, b)
    }

    fn assert_readable(palette: &Palette, bg: Rgb, label: &str) {
        let pairs = [
            (
                "focused tab",
                palette.surface_dim,
                palette.accent,
                ACCENT_TEXT_CONTRAST,
            ),
            (
                "inactive tab",
                palette.overlay0,
                palette.surface0,
                MUTED_TEXT_CONTRAST,
            ),
            ("active row", palette.text, palette.active_row_bg, 4.5),
            (
                "active row detail",
                palette.overlay0,
                palette.active_row_bg,
                MUTED_TEXT_CONTRAST,
            ),
            ("cursor row", palette.text, palette.selection_bg, 4.5),
            (
                "cursor row detail",
                palette.subtext0,
                palette.selection_bg,
                MUTED_TEXT_CONTRAST,
            ),
        ];
        for (name, text, surface, minimum) in pairs {
            let ratio = contrast(as_rgb(text), as_rgb(surface));
            assert!(ratio >= minimum, "{label}: {name} is {ratio:.2}:1");
        }
        for (name, state) in [
            ("green", palette.green),
            ("yellow", palette.yellow),
            ("red", palette.red),
            ("blue", palette.blue),
            ("teal", palette.teal),
        ] {
            let ratio = contrast(as_rgb(state), bg);
            assert!(ratio >= STATE_CONTRAST, "{label}: {name} is {ratio:.2}:1");
        }
        let row = contrast(as_rgb(palette.active_row_bg), bg);
        assert!(
            row >= 1.05,
            "{label}: active row blends into background ({row:.2}:1)"
        );
    }

    #[test]
    fn needs_both_default_colors() {
        let mut theme = host((0, 0, 0), (255, 255, 255), &[]);
        theme.foreground = None;
        assert_eq!(Palette::terminal_adapted(&theme), None);
    }

    #[test]
    fn light_scheme_with_pale_ansi_colors_stays_readable() {
        let bg = (239, 233, 214);
        let theme = host(
            (80, 85, 90),
            bg,
            &[
                (ANSI_RED, (220, 140, 140)),
                (ANSI_GREEN, (170, 200, 150)),
                (ANSI_YELLOW, (230, 210, 120)),
                (ANSI_BLUE, (84, 145, 192)),
                (ANSI_CYAN, (140, 200, 200)),
            ],
        );
        let palette = Palette::terminal_adapted(&theme).unwrap();
        assert_readable(&palette, bg, "light");
    }

    #[test]
    fn dark_scheme_stays_readable() {
        let bg = (30, 30, 40);
        let theme = host(
            (210, 210, 200),
            bg,
            &[(ANSI_BLUE, (40, 60, 160)), (ANSI_RED, (120, 30, 30))],
        );
        let palette = Palette::terminal_adapted(&theme).unwrap();
        assert_readable(&palette, bg, "dark");
    }

    #[test]
    fn low_contrast_scheme_without_palette_stays_readable() {
        let bg = (120, 120, 120);
        let theme = host((150, 150, 150), bg, &[]);
        let palette = Palette::terminal_adapted(&theme).unwrap();
        assert_readable(&palette, bg, "low contrast");
    }
}
