use tracing::{debug, instrument};

use crate::session::SessionHandle;

/// Thin wrappers around `zellij action …` CLI calls.
///
/// All methods accept an optional `SessionHandle`; when present the
/// `--session <name>` flag is prepended so the command targets the right
/// session even when the caller is not running inside Zellij.
pub struct ZellijActions;

impl ZellijActions {
    /// Open a new pane and run `command` inside it.
    /// Returns the pane ID (parsed from zellij's `terminal_<id>` stdout).
    ///
    /// `rally agent spawn --workspace X -- claude ...` translates to:
    /// `zellij [--session S] action new-pane [--cwd D] --name <name> -- <command>`
    #[instrument(skip_all, fields(cmd = ?command, session = ?handle.as_ref().map(|h| h.session_name.as_str())))]
    pub fn new_pane(
        handle: Option<&SessionHandle>,
        name: Option<&str>,
        cwd: Option<&std::path::Path>,
        command: &[&str],
    ) -> anyhow::Result<u32> {
        let args = Self::new_pane_args(handle, name, cwd, command);
        debug!(?args, "running zellij action new-pane");
        let output = std::process::Command::new("zellij")
            .args(&args)
            .output()
            .map_err(|e| anyhow::anyhow!("zellij new-pane failed: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("zellij action new-pane exited with {}: {stderr}", output.status);
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_pane_id(stdout.trim())
    }

    /// Build the argument list for `new_pane` without executing.
    pub fn new_pane_args(
        handle: Option<&SessionHandle>,
        name: Option<&str>,
        cwd: Option<&std::path::Path>,
        command: &[&str],
    ) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        if let Some(h) = handle {
            args.push("--session".into());
            args.push(h.session_name.to_string());
        }
        args.extend(["action".into(), "new-pane".into()]);
        if let Some(n) = name {
            args.push("--name".into());
            args.push(n.into());
        }
        if let Some(c) = cwd {
            args.push("--cwd".into());
            args.push(c.to_string_lossy().into_owned());
        }
        if !command.is_empty() {
            args.push("--".into());
            args.extend(command.iter().map(|s| (*s).to_string()));
        }
        args
    }

    /// Dump the screen contents of a specific pane by ID.
    ///
    /// Uses `zellij [--session S] action dump-screen --pane-id <id>`
    /// (requires zellij >= 0.44.0). Omitting `--path` prints to stdout.
    #[instrument(skip_all, fields(pane_id, session = ?handle.as_ref().map(|h| h.session_name.as_str())))]
    pub fn dump_screen(handle: Option<&SessionHandle>, pane_id: u32) -> anyhow::Result<String> {
        let args = Self::dump_screen_args(handle, pane_id);
        debug!(pane_id, "running zellij action dump-screen --pane-id");
        let output = std::process::Command::new("zellij")
            .args(&args)
            .output()
            .map_err(|e| anyhow::anyhow!("zellij dump-screen failed: {e}"))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "zellij action dump-screen exited with {}: {stderr}",
                output.status
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    }

    /// Build the argument list for `dump_screen` without executing.
    pub fn dump_screen_args(handle: Option<&SessionHandle>, pane_id: u32) -> Vec<String> {
        let mut args: Vec<String> = Vec::new();
        if let Some(h) = handle {
            args.push("--session".into());
            args.push(h.session_name.to_string());
        }
        args.extend([
            "action".into(),
            "dump-screen".into(),
            "--pane-id".into(),
            pane_id.to_string(),
        ]);
        args
    }

    /// Rename a pane (used after `rally _attach` to label the agent pane).
    #[instrument(skip_all, fields(name, session = ?handle.as_ref().map(|h| h.session_name.as_str())))]
    pub fn rename_pane(handle: Option<&SessionHandle>, name: &str) -> anyhow::Result<()> {
        let mut cmd = std::process::Command::new("zellij");
        if let Some(h) = handle {
            cmd.arg("--session").arg(h.session_name.as_str());
        }
        cmd.args(["action", "rename-pane", name]);
        debug!(name, "running zellij action rename-pane");
        let status = cmd
            .status()
            .map_err(|e| anyhow::anyhow!("zellij rename-pane failed: {e}"))?;
        if !status.success() {
            anyhow::bail!("zellij action rename-pane exited with {status}");
        }
        Ok(())
    }

    /// Focus a specific pane by ID (requires zellij >= 0.44.1).
    #[instrument(skip_all, fields(pane_id, session = ?handle.as_ref().map(|h| h.session_name.as_str())))]
    pub fn focus_pane(handle: Option<&SessionHandle>, pane_id: u32) -> anyhow::Result<()> {
        let mut cmd = std::process::Command::new("zellij");
        if let Some(h) = handle {
            cmd.arg("--session").arg(h.session_name.as_str());
        }
        let pane_id_str = pane_id.to_string();
        cmd.args(["action", "focus-pane-id", &pane_id_str]);
        debug!(pane_id, "running zellij action focus-pane-id");
        let status = cmd
            .status()
            .map_err(|e| anyhow::anyhow!("zellij focus-pane failed: {e}"))?;
        if !status.success() {
            anyhow::bail!("zellij action focus-pane-id exited with {status}");
        }
        Ok(())
    }
    /// Send data to the rally plugin via `zellij pipe`.
    /// Targets a specific session if provided, otherwise broadcasts.
    pub fn pipe_to_plugin(session_name: &str, payload: &str) -> anyhow::Result<()> {
        let status = std::process::Command::new("zellij")
            .arg("--session")
            .arg(session_name)
            .arg("pipe")
            .arg("--name")
            .arg("state_snapshot")
            .arg("--")
            .arg(payload)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();

        match status {
            Ok(s) if s.success() => Ok(()),
            Ok(s) => {
                debug!(session = session_name, exit = ?s.code(), "zellij pipe failed");
                Ok(())
            }
            Err(e) => {
                debug!(error = %e, "zellij pipe command not found or failed to start");
                Ok(())
            }
        }
    }

    /// List active Zellij session names. Returns empty vec on error.
    pub fn list_sessions() -> Vec<String> {
        let output = std::process::Command::new("zellij")
            .args(["list-sessions", "--no-formatting", "--short"])
            .output();

        match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.trim().to_string())
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// Parse a pane ID from zellij's stdout format: `terminal_<id>` or `plugin_<id>`.
fn parse_pane_id(output: &str) -> anyhow::Result<u32> {
    let id_str = output
        .strip_prefix("terminal_")
        .or_else(|| output.strip_prefix("plugin_"))
        .ok_or_else(|| anyhow::anyhow!("unexpected pane ID format: {output:?}"))?;
    id_str
        .parse()
        .map_err(|e| anyhow::anyhow!("failed to parse pane ID from {output:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::{DetectedVia, SessionHandle};
    use compact_str::CompactString;

    fn session(name: &str) -> SessionHandle {
        SessionHandle {
            session_name: CompactString::from(name),
            detected_via: DetectedVia::Standalone,
            owned: true,
        }
    }

    #[test]
    fn new_pane_args_without_session() {
        let args =
            ZellijActions::new_pane_args(None, Some("my-agent"), None, &["claude", "--chat"]);
        assert_eq!(
            args,
            vec!["action", "new-pane", "--name", "my-agent", "--", "claude", "--chat"]
        );
    }

    #[test]
    fn new_pane_args_with_session_and_cwd() {
        let handle = session("rally-demo");
        let args = ZellijActions::new_pane_args(
            Some(&handle),
            Some("impl"),
            Some(std::path::Path::new("/home/user/project")),
            &["bash"],
        );
        assert_eq!(
            args,
            vec![
                "--session",
                "rally-demo",
                "action",
                "new-pane",
                "--name",
                "impl",
                "--cwd",
                "/home/user/project",
                "--",
                "bash"
            ]
        );
    }

    #[test]
    fn new_pane_args_empty_command() {
        let args = ZellijActions::new_pane_args(None, None, None, &[]);
        assert_eq!(args, vec!["action", "new-pane"]);
    }

    #[test]
    fn dump_screen_args_without_session() {
        let args = ZellijActions::dump_screen_args(None, 42);
        assert_eq!(args, vec!["action", "dump-screen", "--pane-id", "42"]);
    }

    #[test]
    fn dump_screen_args_with_session() {
        let handle = session("rally-demo");
        let args = ZellijActions::dump_screen_args(Some(&handle), 7);
        assert_eq!(
            args,
            vec![
                "--session",
                "rally-demo",
                "action",
                "dump-screen",
                "--pane-id",
                "7"
            ]
        );
    }

    #[test]
    fn parse_terminal_pane_id() {
        assert_eq!(parse_pane_id("terminal_42").unwrap(), 42);
    }

    #[test]
    fn parse_plugin_pane_id() {
        assert_eq!(parse_pane_id("plugin_7").unwrap(), 7);
    }

    #[test]
    fn parse_pane_id_rejects_garbage() {
        assert!(parse_pane_id("unknown_format").is_err());
        assert!(parse_pane_id("terminal_abc").is_err());
        assert!(parse_pane_id("").is_err());
    }
}
