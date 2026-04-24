use compact_str::CompactString;
use tracing::{debug, info, warn};

/// How the Zellij session was discovered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectedVia {
    /// Found via `ZELLIJ_SESSION_NAME` environment variable (plugin mode —
    /// the CLI is running inside an existing Zellij session).
    EnvVar,
    /// Rally spawned and owns this session (`rally up`).
    Standalone,
}

/// A handle to a Zellij session. Passed to `ZellijActions` for all CLI calls.
#[derive(Debug, Clone)]
pub struct SessionHandle {
    pub session_name: CompactString,
    pub detected_via: DetectedVia,
    /// True when rally owns the session lifecycle (spawned via `rally up`).
    pub owned: bool,
}

/// Detects a Zellij session from the environment.
///
/// Reads `ZELLIJ_SESSION_NAME`. Returns `None` when the CLI is not running
/// inside a Zellij session (standalone mode or bare shell).
pub struct PluginBootstrap;

impl PluginBootstrap {
    pub fn detect() -> Option<SessionHandle> {
        match std::env::var("ZELLIJ_SESSION_NAME") {
            Ok(name) if !name.is_empty() => {
                let handle = SessionHandle {
                    session_name: CompactString::from(name.as_str()),
                    detected_via: DetectedVia::EnvVar,
                    owned: false,
                };
                debug!(session = %handle.session_name, "detected zellij session from env");
                Some(handle)
            }
            Ok(_) => {
                warn!("ZELLIJ_SESSION_NAME is set but empty");
                None
            }
            Err(_) => {
                debug!("not inside a zellij session (ZELLIJ_SESSION_NAME not set)");
                None
            }
        }
    }
}

/// Spawns and owns a dedicated Zellij session for a workspace (`rally up`).
pub struct StandaloneBootstrap;

impl StandaloneBootstrap {
    /// Replace the current process with `zellij attach -c <session_name>`.
    /// The user is dropped directly into the session. Does not return on success.
    pub fn up(session_name: &str, layout_path: Option<&std::path::Path>) -> anyhow::Result<()> {
        use std::os::unix::process::CommandExt;

        let mut cmd = std::process::Command::new("zellij");
        cmd.arg("attach").arg("-c").arg(session_name);
        if let Some(layout) = layout_path {
            cmd.arg("--layout").arg(layout);
        }

        info!(
            session = session_name,
            "exec'ing into standalone zellij session"
        );
        let err = cmd.exec(); // only returns on error
        Err(anyhow::anyhow!("failed to exec zellij: {err}"))
    }

    /// Kill a rally-owned Zellij session (`rally down`).
    pub fn down(session_name: &str) -> anyhow::Result<()> {
        info!(
            session = session_name,
            "tearing down standalone zellij session"
        );
        let status = std::process::Command::new("zellij")
            .args(["delete-session", session_name, "--force"])
            .status()
            .map_err(|e| anyhow::anyhow!("failed to run zellij delete-session: {e}"))?;

        if !status.success() {
            warn!(
                session = session_name,
                "zellij delete-session returned non-zero"
            );
        }
        Ok(())
    }
}
