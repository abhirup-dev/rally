use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

use tracing::{debug, warn};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute, style};

const ACTIONS: &[(&str, &str)] = &[
    ("Split right", "Open a new terminal to the right"),
    ("Split down", "Open a new terminal below"),
    ("Restart", "Close and relaunch terminal in same CWD"),
];

pub fn run(pane_id: u32, cwd: Option<PathBuf>) -> anyhow::Result<()> {
    let cwd_str = cwd
        .as_deref()
        .and_then(|p| p.to_str())
        .unwrap_or("~");

    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, cursor::Hide)?;

    let result = menu_loop(&mut stdout, cwd_str);

    execute!(stdout, cursor::Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;

    match result? {
        Some(action) => execute_action(action, pane_id, cwd_str),
        None => Ok(()),
    }
}

fn menu_loop(stdout: &mut io::Stdout, cwd_str: &str) -> anyhow::Result<Option<usize>> {
    let mut selected: usize = 0;

    loop {
        draw(stdout, selected, cwd_str)?;

        if let Event::Key(key) = event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    selected = selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Char('j')
                    if selected + 1 < ACTIONS.len() =>
                {
                    selected += 1;
                }
                KeyCode::Enter => return Ok(Some(selected)),
                KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
                _ => {}
            }
        }
    }
}

fn draw(stdout: &mut io::Stdout, selected: usize, cwd_str: &str) -> anyhow::Result<()> {
    // In raw mode, '\n' moves down without carriage-return. Place each line
    // with MoveTo(0, row) instead of writeln! to keep columns clean.
    execute!(stdout, terminal::Clear(terminal::ClearType::All))?;

    let mut row: u16 = 0;
    execute!(stdout, cursor::MoveTo(0, row))?;
    write!(stdout, " {}Pane Actions{}", style::Attribute::Bold, style::Attribute::Reset)?;
    row += 1;

    execute!(stdout, cursor::MoveTo(0, row))?;
    write!(stdout, " CWD: {cwd_str}")?;
    row += 2;

    for (i, (label, desc)) in ACTIONS.iter().enumerate() {
        execute!(stdout, cursor::MoveTo(0, row))?;
        if i == selected {
            write!(stdout, " \x1b[7m > {label:16}\x1b[27m  {desc}")?;
        } else {
            write!(stdout, "   {label:16}  \x1b[2m{desc}\x1b[22m")?;
        }
        row += 1;
    }
    row += 1;

    execute!(stdout, cursor::MoveTo(0, row))?;
    write!(stdout, " \x1b[2m[j/k] move  [Enter] select  [Esc/q] cancel\x1b[22m")?;
    stdout.flush()?;
    Ok(())
}

fn execute_action(action: usize, pane_id: u32, cwd_str: &str) -> anyhow::Result<()> {
    match action {
        0 => zellij_split("right", cwd_str),
        1 => zellij_split("down", cwd_str),
        2 => zellij_restart(pane_id, cwd_str),
        _ => Ok(()),
    }
}

fn zellij_split(direction: &str, cwd_str: &str) -> anyhow::Result<()> {
    debug!(direction, cwd_str, "pane menu: split");
    let status = Command::new("zellij")
        .args(["action", "new-pane", "--direction", direction, "--cwd", cwd_str])
        .status()?;
    if !status.success() {
        warn!(direction, cwd_str, ?status, "zellij new-pane failed");
        anyhow::bail!("zellij new-pane failed: {status}");
    }
    Ok(())
}

fn zellij_restart(pane_id: u32, cwd_str: &str) -> anyhow::Result<()> {
    debug!(pane_id, cwd_str, "pane menu: restart");
    let pane_id_str = pane_id.to_string();

    let status = Command::new("zellij")
        .args(["action", "focus-pane-id", &pane_id_str])
        .status()?;
    if !status.success() {
        warn!(pane_id, ?status, "focus-pane-id failed — target pane may be stale");
        anyhow::bail!("zellij focus-pane-id {pane_id} failed: {status}");
    }

    let status = Command::new("zellij")
        .args(["action", "close-pane"])
        .status()?;
    if !status.success() {
        warn!(pane_id, ?status, "close-pane failed after focus");
        anyhow::bail!("zellij close-pane failed: {status}");
    }

    let status = Command::new("zellij")
        .args(["action", "new-pane", "--cwd", cwd_str])
        .status()?;
    if !status.success() {
        warn!(cwd_str, ?status, "new-pane after restart failed");
        anyhow::bail!("zellij new-pane failed: {status}");
    }
    Ok(())
}
