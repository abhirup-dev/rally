use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;

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
                KeyCode::Down | KeyCode::Char('j') => {
                    if selected + 1 < ACTIONS.len() {
                        selected += 1;
                    }
                }
                KeyCode::Enter => return Ok(Some(selected)),
                KeyCode::Esc | KeyCode::Char('q') => return Ok(None),
                _ => {}
            }
        }
    }
}

fn draw(stdout: &mut io::Stdout, selected: usize, cwd_str: &str) -> anyhow::Result<()> {
    execute!(stdout, cursor::MoveTo(0, 0), terminal::Clear(terminal::ClearType::All))?;

    writeln!(stdout, "{}", style::Attribute::Bold)?;
    writeln!(stdout, " Pane Actions")?;
    writeln!(stdout, "{}", style::Attribute::Reset)?;
    writeln!(stdout, " CWD: {cwd_str}")?;
    writeln!(stdout)?;

    for (i, (label, desc)) in ACTIONS.iter().enumerate() {
        if i == selected {
            writeln!(stdout, " \x1b[7m > {label:16}\x1b[27m  {desc}")?;
        } else {
            writeln!(stdout, "   {label:16}  \x1b[2m{desc}\x1b[22m")?;
        }
    }

    writeln!(stdout)?;
    writeln!(stdout, " \x1b[2m[j/k] move  [Enter] select  [Esc] cancel\x1b[22m")?;
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
    let status = Command::new("zellij")
        .args(["action", "new-pane", "--direction", direction, "--cwd", cwd_str])
        .status()?;
    if !status.success() {
        anyhow::bail!("zellij new-pane failed: {status}");
    }
    Ok(())
}

fn zellij_restart(pane_id: u32, cwd_str: &str) -> anyhow::Result<()> {
    let pane_id_str = pane_id.to_string();

    // 1. Focus the target pane (by ID) so close-pane targets it.
    let status = Command::new("zellij")
        .args(["action", "focus-pane-id", &pane_id_str])
        .status()?;
    if !status.success() {
        anyhow::bail!("zellij focus-pane-id {pane_id} failed: {status}");
    }

    // 2. Close the now-focused target pane.
    let status = Command::new("zellij")
        .args(["action", "close-pane"])
        .status()?;
    if !status.success() {
        anyhow::bail!("zellij close-pane failed: {status}");
    }

    // 3. Open a new terminal in the same CWD.
    let status = Command::new("zellij")
        .args(["action", "new-pane", "--cwd", cwd_str])
        .status()?;
    if !status.success() {
        anyhow::bail!("zellij new-pane failed: {status}");
    }
    Ok(())
}
