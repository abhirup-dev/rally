mod ipc;
mod services;
mod tracing_init;

use std::sync::Arc;

use rally_config::RallyConfig;
use rally_events::EventBus;
use rally_store::Store;
use tracing::{error, info};

#[tokio::main]
async fn main() {
    let _guard = tracing_init::init();

    if let Err(e) = run().await {
        error!(error = %e, "rallyd fatal error");
        eprintln!("rallyd: {e}");
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let config = RallyConfig::load()?;
    let socket_path = config.socket_path();

    // Ensure parent directory exists
    if let Some(parent) = socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Remove stale socket file
    if socket_path.exists() {
        std::fs::remove_file(&socket_path)?;
    }

    // Open database
    let db_path = data_dir().join("state.db");
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let store = Store::open(&db_path)?;
    info!(db = %db_path.display(), "store opened");

    let event_bus = EventBus::new();
    let service = Arc::new(services::RallyService::new(store, event_bus));

    // Bind unix socket
    let listener = tokio::net::UnixListener::bind(&socket_path)?;
    info!(socket = %socket_path.display(), "rallyd listening");

    // Write pid file
    let pid_path = socket_path.with_extension("pid");
    std::fs::write(&pid_path, std::process::id().to_string())?;
    info!(pid_file = %pid_path.display(), pid = std::process::id(), "pid file written");

    // Serve
    ipc::serve(listener, service).await;

    Ok(())
}

fn data_dir() -> std::path::PathBuf {
    if let Ok(data_home) = std::env::var("XDG_DATA_HOME") {
        return std::path::PathBuf::from(data_home).join("rally");
    }
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home).join(".local/share/rally");
    }
    std::path::PathBuf::from("/tmp/rally/data")
}
