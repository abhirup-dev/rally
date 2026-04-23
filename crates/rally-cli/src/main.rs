mod ipc_client;
mod tracing_init;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use rally_config::RallyConfig;
use rally_proto::v1::{Request, Response};
use tracing::{debug, error, info, warn};

use crate::ipc_client::IpcClient;

#[derive(Parser)]
#[command(name = "rally", about = "Terminal-native multi-agent orchestrator")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Output as JSON
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Manage workspaces
    Workspace {
        #[command(subcommand)]
        action: WorkspaceAction,
    },
    /// Manage agents
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Session and daemon management
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
}

#[derive(Subcommand)]
enum WorkspaceAction {
    /// Create a new workspace
    New {
        #[arg(long)]
        name: String,
        #[arg(long)]
        repo: Option<PathBuf>,
    },
    /// List workspaces
    Ls,
    /// Show workspace details
    Show {
        id: String,
    },
}

#[derive(Subcommand)]
enum AgentAction {
    /// Register a new agent
    Spawn {
        #[arg(long)]
        workspace: String,
        #[arg(long)]
        role: String,
        #[arg(long, default_value = "claude-code")]
        runtime: String,
    },
    /// List agents
    Ls {
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Show agent details
    Show {
        id: String,
    },
}

#[derive(Subcommand)]
enum SessionAction {
    /// Show daemon status
    Status,
}

#[tokio::main]
async fn main() {
    let _guard = tracing_init::init();
    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        error!(error = %e, "CLI error");
        eprintln!("rally: {e}");
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let config = RallyConfig::load()?;
    let socket_path = config.socket_path();

    let command_name = match &cli.command {
        Commands::Workspace { action } => match action {
            WorkspaceAction::New { .. } => "workspace new",
            WorkspaceAction::Ls => "workspace ls",
            WorkspaceAction::Show { .. } => "workspace show",
        },
        Commands::Agent { action } => match action {
            AgentAction::Spawn { .. } => "agent spawn",
            AgentAction::Ls { .. } => "agent ls",
            AgentAction::Show { .. } => "agent show",
        },
        Commands::Session { action } => match action {
            SessionAction::Status => "session status",
        },
    };
    debug!(command = command_name, json = cli.json, "cli dispatched");

    match cli.command {
        Commands::Workspace { action } => {
            let mut client = connect(&socket_path).await?;
            match action {
                WorkspaceAction::New { name, repo } => {
                    let resp = client.call(Request::CreateWorkspace {
                        name: name.into(),
                        repo,
                    }).await?;
                    print_response(&resp, cli.json);
                }
                WorkspaceAction::Ls => {
                    let resp = client.call(Request::ListWorkspaces).await?;
                    print_response(&resp, cli.json);
                }
                WorkspaceAction::Show { id } => {
                    let ws_id = parse_workspace_id(&id)?;
                    let resp = client.call(Request::GetWorkspace { id: ws_id }).await?;
                    print_response(&resp, cli.json);
                }
            }
        }
        Commands::Agent { action } => {
            let mut client = connect(&socket_path).await?;
            match action {
                AgentAction::Spawn { workspace, role, runtime } => {
                    let ws_id = parse_workspace_id(&workspace)?;
                    let resp = client.call(Request::RegisterAgent {
                        workspace_id: ws_id,
                        role: role.into(),
                        runtime: runtime.into(),
                    }).await?;
                    print_response(&resp, cli.json);
                }
                AgentAction::Ls { workspace } => {
                    let ws_id = workspace.as_deref().map(parse_workspace_id).transpose()?;
                    let resp = client.call(Request::ListAgents { workspace_id: ws_id }).await?;
                    print_response(&resp, cli.json);
                }
                AgentAction::Show { id } => {
                    let agent_id = parse_agent_id(&id)?;
                    let resp = client.call(Request::GetAgent { id: agent_id }).await?;
                    print_response(&resp, cli.json);
                }
            }
        }
        Commands::Session { action } => {
            match action {
                SessionAction::Status => {
                    if socket_path.exists() {
                        let pid_path = socket_path.with_extension("pid");
                        let pid = std::fs::read_to_string(&pid_path).unwrap_or_default();
                        if cli.json {
                            println!(r#"{{"status":"running","socket":"{}","pid":{}}}"#,
                                socket_path.display(), pid.trim());
                        } else {
                            println!("rallyd: running (pid {})", pid.trim());
                            println!("socket: {}", socket_path.display());
                        }
                    } else if cli.json {
                        println!(r#"{{"status":"stopped"}}"#);
                    } else {
                        println!("rallyd: not running");
                    }
                }
            }
        }
    }

    Ok(())
}

async fn connect(socket_path: &std::path::Path) -> anyhow::Result<IpcClient> {
    debug!(socket = %socket_path.display(), "connecting to daemon");
    match IpcClient::connect(socket_path).await {
        Ok(c) => Ok(c),
        Err(_) => {
            eprintln!("rally: daemon not running, starting rallyd...");
            autostart_daemon(socket_path)?;
            IpcClient::connect(socket_path).await.map_err(|e| {
                eprintln!("rally: still cannot connect after autostart");
                e
            })
        }
    }
}

fn autostart_daemon(socket_path: &std::path::Path) -> anyhow::Result<()> {
    use std::process::Command;

    let rallyd = std::env::current_exe()?
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .join("rallyd");

    if !rallyd.exists() {
        anyhow::bail!("rallyd binary not found at {}", rallyd.display());
    }

    // Spawn detached
    let mut cmd = Command::new(&rallyd);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());

    // Set RALLY_DAEMON_SOCKET_PATH so child uses the same socket
    cmd.env("RALLY_DAEMON_SOCKET_PATH", socket_path);

    info!(rallyd_path = %rallyd.display(), socket = %socket_path.display(), "autostarting rallyd");
    let _child = cmd.spawn()?;

    // Wait for socket to appear (up to 3 seconds)
    for _ in 0..30 {
        if socket_path.exists() {
            std::thread::sleep(std::time::Duration::from_millis(50));
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    warn!(attempted_ms = 3000, "rallyd did not start");
    anyhow::bail!("rallyd did not start within 3 seconds")
}

fn print_response(resp: &Response, json: bool) {
    if json {
        println!("{}", serde_json::to_string_pretty(resp).unwrap_or_default());
    } else {
        match resp {
            Response::Ok => println!("ok"),
            Response::Error { message } => {
                eprintln!("error: {message}");
                std::process::exit(1);
            }
            Response::Workspace(ws) => {
                println!("{:<12} {}", "id:", ws.id);
                println!("{:<12} {}", "name:", ws.name);
                println!("{:<12} {}", "key:", ws.canonical_key);
                if let Some(ref repo) = ws.repo {
                    println!("{:<12} {}", "repo:", repo.display());
                }
            }
            Response::WorkspaceList { items: list } => {
                if list.is_empty() {
                    println!("no workspaces");
                } else {
                    for ws in list {
                        println!("{} {} ({})", ws.id, ws.name, ws.canonical_key);
                    }
                }
            }
            Response::Agent(a) => {
                println!("{:<12} {}", "id:", a.id);
                println!("{:<12} {}", "workspace:", a.workspace_id);
                println!("{:<12} {}", "role:", a.role);
                println!("{:<12} {}", "runtime:", a.runtime);
                println!("{:<12} {:?}", "state:", a.state);
            }
            Response::AgentList { items: list } => {
                if list.is_empty() {
                    println!("no agents");
                } else {
                    for a in list {
                        println!("{} {} {:?} (ws:{})", a.id, a.role, a.state, a.workspace_id);
                    }
                }
            }
            Response::InboxList { items: list } => {
                if list.is_empty() {
                    println!("no inbox items");
                } else {
                    for item in list {
                        println!("{} {:?} acked:{}", item.id, item.urgency, item.acked);
                    }
                }
            }
            _ => {
                println!("{}", serde_json::to_string_pretty(resp).unwrap_or_default());
            }
        }
    }
}

fn parse_workspace_id(s: &str) -> anyhow::Result<rally_proto::v1::WorkspaceId> {
    let ulid: ulid::Ulid = s.parse().map_err(|e| anyhow::anyhow!("invalid workspace id '{s}': {e}"))?;
    Ok(rally_proto::v1::WorkspaceId::new(ulid))
}

fn parse_agent_id(s: &str) -> anyhow::Result<rally_proto::v1::AgentId> {
    let ulid: ulid::Ulid = s.parse().map_err(|e| anyhow::anyhow!("invalid agent id '{s}': {e}"))?;
    Ok(rally_proto::v1::AgentId::new(ulid))
}
