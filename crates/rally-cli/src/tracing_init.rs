use std::path::PathBuf;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling;
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::EnvFilter;

/// Initialise the global tracing subscriber for the `rally` CLI.
///
/// Call once at the very top of `main`. Hold the returned `WorkerGuard` for
/// the process lifetime — dropping it flushes and closes the log file.
///
/// Log verbosity is controlled by `RALLY_LOG` (default: `rally=warn`).
/// CLI default is `warn` (quieter than daemon) to avoid polluting stdout.
/// Log file: `~/.local/state/rally/logs/rally-cli.log` (daily rolling).
pub fn init() -> WorkerGuard {
    let log_dir = log_dir();
    std::fs::create_dir_all(&log_dir).ok();

    let file_appender = rolling::daily(&log_dir, "rally-cli.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let env_filter =
        EnvFilter::try_from_env("RALLY_LOG").unwrap_or_else(|_| EnvFilter::new("rally=warn"));

    tracing_subscriber::registry()
        .with(env_filter)
        .with(fmt::layer().with_writer(non_blocking).with_target(true))
        .init();

    guard
}

fn log_dir() -> PathBuf {
    if let Ok(state_home) = std::env::var("XDG_STATE_HOME") {
        return PathBuf::from(state_home).join("rally/logs");
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local/state/rally/logs");
    }
    PathBuf::from("/tmp/rally/logs")
}
