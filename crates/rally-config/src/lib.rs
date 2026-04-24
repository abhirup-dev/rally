mod jsonc;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

pub use crate::jsonc::strip_jsonc_comments;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    ReadFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse config: {0}")]
    Parse(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// Top-level config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RallyConfig {
    pub daemon: DaemonConfig,
    pub zellij: ZellijConfig,
    pub worktree: WorktreeConfig,
    pub mcp: McpConfig,
    pub capture: CaptureConfig,
}

impl Default for RallyConfig {
    fn default() -> Self {
        Self {
            daemon: DaemonConfig::default(),
            zellij: ZellijConfig::default(),
            worktree: WorktreeConfig::default(),
            mcp: McpConfig::default(),
            capture: CaptureConfig::default(),
        }
    }
}

// ---------------------------------------------------------------------------
// Section structs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DaemonConfig {
    pub socket_path: Option<PathBuf>,
    pub log_level: String,
    pub log_file: Option<PathBuf>,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            socket_path: None,
            log_level: "info".into(),
            log_file: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ZellijConfig {
    pub binary: String,
    pub default_layout: Option<PathBuf>,
    pub plugin_path: Option<PathBuf>,
}

impl Default for ZellijConfig {
    fn default() -> Self {
        Self {
            binary: "zellij".into(),
            default_layout: None,
            plugin_path: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorktreeConfig {
    pub backend: String,
    pub worktrunk_binary: String,
}

impl Default for WorktreeConfig {
    fn default() -> Self {
        Self {
            backend: "worktrunk".into(),
            worktrunk_binary: "wt".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct McpConfig {
    pub default_transport: String,
    pub http_port: u16,
    pub allow_control: bool,
}

impl Default for McpConfig {
    fn default() -> Self {
        Self {
            default_transport: "stdio".into(),
            http_port: 8377,
            allow_control: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CaptureConfig {
    pub ring_buffer_mb: u32,
    pub poll_hz: u32,
    pub snapshot_format: String,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            ring_buffer_mb: 16,
            poll_hz: 5,
            snapshot_format: "ansi".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Loading + layered merge
// ---------------------------------------------------------------------------

impl RallyConfig {
    /// Resolve the config file path: `RALLY_CONFIG` env → `~/.config/rally/config.jsonc`.
    pub fn config_file_path() -> Option<PathBuf> {
        if let Ok(p) = std::env::var("RALLY_CONFIG") {
            return Some(PathBuf::from(p));
        }
        if let Ok(config_home) = std::env::var("XDG_CONFIG_HOME") {
            return Some(PathBuf::from(config_home).join("rally/config.jsonc"));
        }
        if let Ok(home) = std::env::var("HOME") {
            return Some(PathBuf::from(home).join(".config/rally/config.jsonc"));
        }
        None
    }

    /// Resolve the daemon socket path.
    pub fn socket_path(&self) -> PathBuf {
        if let Some(ref p) = self.daemon.socket_path {
            return p.clone();
        }
        if let Ok(runtime) = std::env::var("XDG_RUNTIME_DIR") {
            return PathBuf::from(runtime).join("rally/rally.sock");
        }
        PathBuf::from("/tmp/rally/rally.sock")
    }

    /// Resolve the daemon log file path.
    pub fn log_file_path(&self) -> PathBuf {
        if let Some(ref p) = self.daemon.log_file {
            return p.clone();
        }
        if let Ok(state_home) = std::env::var("XDG_STATE_HOME") {
            return PathBuf::from(state_home).join("rally/rallyd.log");
        }
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(".local/state/rally/rallyd.log");
        }
        PathBuf::from("/tmp/rally/rallyd.log")
    }

    /// Load config with layered merge: defaults → file → env overrides.
    ///
    /// If no config file exists, returns defaults with env overrides applied.
    pub fn load() -> Result<Self, ConfigError> {
        let mut config = Self::default();

        // Layer 1: file (if it exists)
        if let Some(path) = Self::config_file_path() {
            if path.exists() {
                info!(path = %path.display(), "loading config file");
                config = Self::load_file(&path)?;
            } else {
                debug!(path = %path.display(), "config file not found, using defaults");
            }
        }

        // Layer 2: env var overrides
        config.apply_env_overrides();

        Ok(config)
    }

    /// Parse a JSONC config file into `RallyConfig`.
    pub fn load_file(path: &Path) -> Result<Self, ConfigError> {
        let raw = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadFile {
            path: path.to_owned(),
            source: e,
        })?;
        let stripped = strip_jsonc_comments(&raw);
        let config: Self = serde_json::from_str(&stripped)?;
        Ok(config)
    }

    /// Apply env var overrides. Convention: `RALLY_DAEMON_LOG_LEVEL`, etc.
    fn apply_env_overrides(&mut self) {
        if let Ok(v) = std::env::var("RALLY_DAEMON_LOG_LEVEL") {
            debug!(value = %v, "env override: RALLY_DAEMON_LOG_LEVEL");
            self.daemon.log_level = v;
        }
        if let Ok(v) = std::env::var("RALLY_DAEMON_SOCKET_PATH") {
            debug!(value = %v, "env override: RALLY_DAEMON_SOCKET_PATH");
            self.daemon.socket_path = Some(PathBuf::from(v));
        }
        if let Ok(v) = std::env::var("RALLY_MCP_ALLOW_CONTROL") {
            debug!(value = %v, "env override: RALLY_MCP_ALLOW_CONTROL");
            self.mcp.allow_control = v == "true" || v == "1";
        }
        if let Ok(v) = std::env::var("RALLY_MCP_HTTP_PORT") {
            match v.parse() {
                Ok(port) => {
                    debug!(value = port, "env override: RALLY_MCP_HTTP_PORT");
                    self.mcp.http_port = port;
                }
                Err(_) => warn!(value = %v, var = "RALLY_MCP_HTTP_PORT", "invalid value, ignoring"),
            }
        }
        if let Ok(v) = std::env::var("RALLY_CAPTURE_RING_BUFFER_MB") {
            match v.parse() {
                Ok(mb) => {
                    debug!(value = mb, "env override: RALLY_CAPTURE_RING_BUFFER_MB");
                    self.capture.ring_buffer_mb = mb;
                }
                Err(_) => {
                    warn!(value = %v, var = "RALLY_CAPTURE_RING_BUFFER_MB", "invalid value, ignoring")
                }
            }
        }
    }
}
