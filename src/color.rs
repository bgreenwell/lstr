//! Centralized color conversions between the three color models in use:
//! `lscolors` (what the `LS_COLORS` environment variable parses to),
//! `colored` (classic-mode terminal output), and `ratatui` (the TUI).
//!
//! All cross-model mapping lives here so the models cannot drift apart.

use lscolors::{Color as LsColor, Style as LsStyle};
use ratatui::style::{Color as TuiColor, Modifier, Style as TuiStyle};

/// Converts an `lscolors` color to a `colored` color for classic-mode output.
pub fn ls_to_colored(color: LsColor) -> colored::Color {
    match color {
        LsColor::Black => colored::Color::Black,
        LsColor::Red => colored::Color::Red,
        LsColor::Green => colored::Color::Green,
        LsColor::Yellow => colored::Color::Yellow,
        LsColor::Blue => colored::Color::Blue,
        LsColor::Magenta => colored::Color::Magenta,
        LsColor::Cyan => colored::Color::Cyan,
        LsColor::White => colored::Color::White,
        LsColor::BrightBlack => colored::Color::BrightBlack,
        LsColor::BrightRed => colored::Color::BrightRed,
        LsColor::BrightGreen => colored::Color::BrightGreen,
        LsColor::BrightYellow => colored::Color::BrightYellow,
        LsColor::BrightBlue => colored::Color::BrightBlue,
        LsColor::BrightMagenta => colored::Color::BrightMagenta,
        LsColor::BrightCyan => colored::Color::BrightCyan,
        LsColor::BrightWhite => colored::Color::BrightWhite,
        // `colored` has no 256-color type; fall back to the default color.
        LsColor::Fixed(_) => colored::Color::White,
        LsColor::RGB(r, g, b) => colored::Color::TrueColor { r, g, b },
    }
}

/// Converts an `lscolors` style (color + font attributes) to a ratatui style
/// for the TUI.
pub fn ls_to_ratatui_style(ls_style: LsStyle) -> TuiStyle {
    let mut style = TuiStyle::default();

    if let Some(fg) = ls_style.foreground {
        style = style.fg(match fg {
            LsColor::Black => TuiColor::Black,
            LsColor::Red => TuiColor::Red,
            LsColor::Green => TuiColor::Green,
            LsColor::Yellow => TuiColor::Yellow,
            LsColor::Blue => TuiColor::Blue,
            LsColor::Magenta => TuiColor::Magenta,
            LsColor::Cyan => TuiColor::Cyan,
            LsColor::White => TuiColor::White,
            LsColor::BrightBlack => TuiColor::Gray,
            LsColor::BrightRed => TuiColor::LightRed,
            LsColor::BrightGreen => TuiColor::LightGreen,
            LsColor::BrightYellow => TuiColor::LightYellow,
            LsColor::BrightBlue => TuiColor::LightBlue,
            LsColor::BrightMagenta => TuiColor::LightMagenta,
            LsColor::BrightCyan => TuiColor::LightCyan,
            LsColor::BrightWhite => TuiColor::White,
            LsColor::Fixed(n) => TuiColor::Indexed(n),
            LsColor::RGB(r, g, b) => TuiColor::Rgb(r, g, b),
        });
    }

    if ls_style.font_style.bold {
        style = style.add_modifier(Modifier::BOLD);
    }
    if ls_style.font_style.italic {
        style = style.add_modifier(Modifier::ITALIC);
    }
    if ls_style.font_style.underline {
        style = style.add_modifier(Modifier::UNDERLINED);
    }

    style
}

/// Converts a `colored` color (used by the icon table) to a ratatui color
/// so the TUI can render the same icon colors as classic mode.
pub fn colored_to_ratatui(color: colored::Color) -> TuiColor {
    match color {
        colored::Color::Black => TuiColor::Black,
        colored::Color::Red => TuiColor::Red,
        colored::Color::Green => TuiColor::Green,
        colored::Color::Yellow => TuiColor::Yellow,
        colored::Color::Blue => TuiColor::Blue,
        colored::Color::Magenta => TuiColor::Magenta,
        colored::Color::Cyan => TuiColor::Cyan,
        colored::Color::White => TuiColor::White,
        colored::Color::BrightBlack => TuiColor::Gray,
        colored::Color::BrightRed => TuiColor::LightRed,
        colored::Color::BrightGreen => TuiColor::LightGreen,
        colored::Color::BrightYellow => TuiColor::LightYellow,
        colored::Color::BrightBlue => TuiColor::LightBlue,
        colored::Color::BrightMagenta => TuiColor::LightMagenta,
        colored::Color::BrightCyan => TuiColor::LightCyan,
        colored::Color::TrueColor { r, g, b } => TuiColor::Rgb(r, g, b),
        _ => TuiColor::Reset,
    }
}
