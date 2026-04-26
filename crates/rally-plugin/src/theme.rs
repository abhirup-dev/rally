use zellij_widgets::prelude::{Modifier, Style};

#[allow(dead_code)]
pub mod palette {
    use zellij_widgets::prelude::Color;

    pub const BASE: Color = Color::Rgb { r: 25, g: 23, b: 36 };
    pub const SURFACE: Color = Color::Rgb { r: 31, g: 29, b: 46 };
    pub const OVERLAY: Color = Color::Rgb { r: 38, g: 35, b: 58 };
    pub const TEXT: Color = Color::Rgb { r: 224, g: 222, b: 244 };
    pub const SUBTLE: Color = Color::Rgb { r: 144, g: 140, b: 170 };
    pub const MUTED: Color = Color::Rgb { r: 110, g: 106, b: 134 };
    pub const ROSE: Color = Color::Rgb { r: 235, g: 188, b: 186 };
    pub const GOLD: Color = Color::Rgb { r: 246, g: 193, b: 119 };
    pub const PINE: Color = Color::Rgb { r: 49, g: 116, b: 143 };
    pub const FOAM: Color = Color::Rgb { r: 156, g: 207, b: 216 };
    pub const IRIS: Color = Color::Rgb { r: 196, g: 167, b: 231 };
    pub const LOVE: Color = Color::Rgb { r: 235, g: 111, b: 146 };
}

/// Visual properties for a single agent state.
pub struct StateTheme {
    pub glyph: &'static str,
    /// Style for sidebar rows (fg color + modifiers).
    pub style: Style,
    /// Hex bg tint for `set_pane_color` (subtle; leaves fg unchanged). `None` = clear tint.
    pub pane_bg: Option<&'static str>,
}

/// Theme for a bare terminal pane (not associated with any Rally agent).
#[allow(dead_code)]
pub fn bare_terminal_theme() -> StateTheme {
    StateTheme {
        glyph: "▪",
        style: Style::default().fg(palette::MUTED),
        pane_bg: None,
    }
}

/// Return the visual theme for a given agent state string (as serialized by the daemon).
pub fn state_theme(state: &str) -> StateTheme {
    match state {
        "initializing" => StateTheme {
            glyph: "◑",
            style: Style::default().fg(palette::GOLD),
            pane_bg: None,
        },
        "running" => StateTheme {
            glyph: "●",
            style: Style::default().fg(palette::PINE),
            pane_bg: Some("#1a2430"),
        },
        "idle" => StateTheme {
            glyph: "○",
            style: Style::default().fg(palette::SUBTLE),
            pane_bg: None,
        },
        "waiting_for_input" => StateTheme {
            glyph: "◉",
            style: Style::default().fg(palette::GOLD),
            pane_bg: Some("#2a2420"),
        },
        "attention_required" => StateTheme {
            glyph: "!",
            style: Style::default()
                .fg(palette::LOVE)
                .add_modifier(Modifier::BOLD),
            pane_bg: Some("#2a1e28"),
        },
        "completed" => StateTheme {
            glyph: "✓",
            style: Style::default()
                .fg(palette::FOAM)
                .add_modifier(Modifier::DIM),
            pane_bg: None,
        },
        "failed" => StateTheme {
            glyph: "✗",
            style: Style::default().fg(palette::LOVE),
            pane_bg: Some("#2a1e28"),
        },
        "stopped" => StateTheme {
            glyph: "─",
            style: Style::default()
                .fg(palette::MUTED)
                .add_modifier(Modifier::DIM),
            pane_bg: None,
        },
        _ => StateTheme {
            glyph: "?",
            style: Style::default().fg(palette::SUBTLE),
            pane_bg: None,
        },
    }
}
