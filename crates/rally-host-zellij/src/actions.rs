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
    ///
    /// `rally agent spawn --workspace X -- claude ...` translates to:
    /// `zellij [--session S] action new-pane [--cwd D] --name <name> -- <command>`
    #[instrument(skip_all, fields(cmd = ?command, session = ?handle.as_ref().map(|h| h.session_name.as_str())))]
    pub fn new_pane(
        handle: Option<&SessionHandle>,
        name: Option<&str>,
        cwd: Option<&std::path::Path>,
        command: &[&str],
    ) -> anyhow::Result<()> {
        let mut args: Vec<&str> = Vec::new();
        if let Some(h) = handle {
            args.extend_from_slice(&["--session", h.session_name.as_str()]);
        }
        args.extend_from_slice(&["action", "new-pane"]);
        if let Some(n) = name {
            args.extend_from_slice(&["--name", n]);
        }
        let cwd_str;
        if let Some(c) = cwd {
            cwd_str = c.to_string_lossy().into_owned();
            args.extend_from_slice(&["--cwd", &cwd_str]);
        }
        if !command.is_empty() {
            args.push("--");
            args.extend_from_slice(command);
        }
        debug!(?args, "running zellij action new-pane");
        let status = std::process::Command::new("zellij")
            .args(&args)
            .status()
            .map_err(|e| anyhow::anyhow!("zellij new-pane failed: {e}"))?;
        if !status.success() {
            anyhow::bail!("zellij action new-pane exited with {status}");
        }
        Ok(())
    }

    /// Dump the screen contents of a specific pane by ID.
    ///
    /// Uses `zellij [--session S] action dump-screen --pane-id <id>`
    /// (requires zellij >= 0.44.0). Omitting `--path` prints to stdout.
    #[instrument(skip_all, fields(pane_id, session = ?handle.as_ref().map(|h| h.session_name.as_str())))]
    pub fn dump_screen(handle: Option<&SessionHandle>, pane_id: u32) -> anyhow::Result<String> {
        let mut cmd = std::process::Command::new("zellij");
        if let Some(h) = handle {
            cmd.arg("--session").arg(h.session_name.as_str());
        }
        let pane_id_str = pane_id.to_string();
        cmd.args(["action", "dump-screen", "--pane-id", &pane_id_str]);
        debug!(pane_id, "running zellij action dump-screen --pane-id");
        let output = cmd
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
        cmd.args(["action", "focus-pane-with-id", &pane_id_str]);
        debug!(pane_id, "running zellij action focus-pane-with-id");
        let status = cmd
            .status()
            .map_err(|e| anyhow::anyhow!("zellij focus-pane failed: {e}"))?;
        if !status.success() {
            anyhow::bail!("zellij action focus-pane-with-id exited with {status}");
        }
        Ok(())
    }
}
