use compact_str::CompactString;
use tracing::{debug, info, warn};

/// Context captured by the `rally _attach` shim from Zellij environment vars.
#[derive(Debug)]
pub struct PaneContext {
    pub session_name: CompactString,
    pub pane_id: u32,
    /// tab_index is not reliably exposed externally; defaults to 0.
    pub tab_index: u32,
}

impl PaneContext {
    /// Read pane correlation data from Zellij environment variables.
    ///
    /// `ZELLIJ_SESSION_NAME` and `ZELLIJ_PANE_ID` are injected by Zellij into
    /// every process running inside a pane.
    pub fn from_env() -> anyhow::Result<Self> {
        Self::from_env_reader(|k| std::env::var(k))
    }

    fn from_env_reader(
        reader: impl Fn(&str) -> Result<String, std::env::VarError>,
    ) -> anyhow::Result<Self> {
        let session_name = reader("ZELLIJ_SESSION_NAME").map_err(|_| {
            anyhow::anyhow!("ZELLIJ_SESSION_NAME not set — is this running inside zellij?")
        })?;
        let pane_id_str = reader("ZELLIJ_PANE_ID").map_err(|_| {
            anyhow::anyhow!("ZELLIJ_PANE_ID not set — is this running inside zellij?")
        })?;
        let pane_id: u32 = pane_id_str
            .trim()
            .parse()
            .map_err(|e| anyhow::anyhow!("invalid ZELLIJ_PANE_ID '{pane_id_str}': {e}"))?;

        debug!(session = %session_name, pane_id, "captured pane context from env");
        Ok(Self {
            session_name: CompactString::from(session_name),
            pane_id,
            tab_index: 0,
        })
    }
}

/// Log the shim correlation at the crosspoint: pane_id ↔ agent_id binding.
pub fn log_attach_correlation(agent_id: &str, ctx: &PaneContext) {
    info!(
        agent_id,
        session = %ctx.session_name,
        pane_id = ctx.pane_id,
        "attach shim: binding pane to agent"
    );
}

/// Warn when the shim cannot read the pane context (e.g. not inside zellij).
pub fn log_attach_env_missing(err: &anyhow::Error) {
    warn!(error = %err, "attach shim: could not read zellij env — pane correlation skipped");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn fake_env(vars: Vec<(&str, &str)>) -> impl Fn(&str) -> Result<String, std::env::VarError> {
        let owned: HashMap<String, String> = vars
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |key| {
            owned
                .get(key)
                .cloned()
                .ok_or(std::env::VarError::NotPresent)
        }
    }

    #[test]
    fn from_env_fails_without_zellij_vars() {
        let env = fake_env(vec![]);
        let result = PaneContext::from_env_reader(env);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("ZELLIJ_SESSION_NAME"),
            "expected session name error, got: {msg}"
        );
    }

    #[test]
    fn from_env_fails_with_malformed_pane_id() {
        let env = fake_env(vec![
            ("ZELLIJ_SESSION_NAME", "test-session"),
            ("ZELLIJ_PANE_ID", "not-a-number"),
        ]);
        let result = PaneContext::from_env_reader(env);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("invalid ZELLIJ_PANE_ID"),
            "expected parse error, got: {msg}"
        );
    }

    #[test]
    fn from_env_succeeds_with_valid_vars() {
        let env = fake_env(vec![
            ("ZELLIJ_SESSION_NAME", "rally-demo"),
            ("ZELLIJ_PANE_ID", "42"),
        ]);
        let ctx = PaneContext::from_env_reader(env).unwrap();
        assert_eq!(ctx.session_name.as_str(), "rally-demo");
        assert_eq!(ctx.pane_id, 42);
        assert_eq!(ctx.tab_index, 0);
    }

    #[test]
    fn from_env_fails_missing_pane_id_only() {
        let env = fake_env(vec![("ZELLIJ_SESSION_NAME", "test-session")]);
        let result = PaneContext::from_env_reader(env);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("ZELLIJ_PANE_ID"),
            "expected pane id error, got: {msg}"
        );
    }
}
