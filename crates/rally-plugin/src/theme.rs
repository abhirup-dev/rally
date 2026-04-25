use zellij_widgets::prelude::{Color, Modifier, Style};

/// Visual properties for a single agent state.
pub struct StateTheme {
    pub glyph: &'static str,
    /// Style for sidebar rows (fg color + modifiers).
    pub style: Style,
    /// Hex bg tint for `set_pane_color` (subtle; leaves fg unchanged). `None` = clear tint.
    pub pane_bg: Option<&'static str>,
}

/// Theme for a bare terminal pane (not associated with any Rally agent).
/// Used by S4.5 / T6 when bare pane nodes appear in the tree.
#[allow(dead_code)]
pub fn bare_terminal_theme() -> StateTheme {
    StateTheme {
        glyph: "▪",
        style: Style::default().fg(Color::DarkGray),
        pane_bg: None,
    }
}

/// Return the visual theme for a given agent state string (as serialized by the daemon).
pub fn state_theme(state: &str) -> StateTheme {
    match state {
        "initializing" => StateTheme {
            glyph: "◐",
            style: Style::default().fg(Color::Yellow),
            pane_bg: None,
        },
        "running" => StateTheme {
            glyph: "●",
            style: Style::default().fg(Color::Green),
            pane_bg: Some("#001800"),
        },
        "idle" => StateTheme {
            glyph: "○",
            style: Style::default().fg(Color::Gray),
            pane_bg: None,
        },
        "waiting_for_input" => StateTheme {
            glyph: "⧗",
            style: Style::default().fg(Color::Yellow),
            pane_bg: Some("#181400"),
        },
        "attention_required" => StateTheme {
            glyph: "◉",
            style: Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
            pane_bg: Some("#1a0000"),
        },
        "completed" => StateTheme {
            glyph: "✓",
            style: Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::DIM),
            pane_bg: None,
        },
        "failed" => StateTheme {
            glyph: "✗",
            style: Style::default().fg(Color::Red),
            pane_bg: Some("#1a0000"),
        },
        "stopped" => StateTheme {
            glyph: "✕",
            style: Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
            pane_bg: None,
        },
        _ => StateTheme {
            glyph: "?",
            style: Style::default(),
            pane_bg: None,
        },
    }
}
