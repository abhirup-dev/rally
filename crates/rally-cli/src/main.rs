#![deny(unsafe_code)]

mod ipc_client;
mod tracing_init;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use rally_capture::CaptureSource;
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
    /// Spawn a rally-owned Zellij session for a workspace
    Up {
        /// Workspace canonical key or alias (e.g. "my-project")
        workspace: String,
    },
    /// Tear down a rally-owned Zellij session
    Down {
        /// Workspace canonical key or alias (e.g. "my-project")
        workspace: String,
    },
    /// Capture pane output
    Capture {
        #[command(subcommand)]
        action: CaptureAction,
    },
    /// Print Zellij plugin installation instructions
    InstallPlugin,
    /// Manage Zellij layouts
    Layout {
        #[command(subcommand)]
        action: LayoutAction,
    },
    /// Manage workspace aliases
    Alias {
        #[command(subcommand)]
        action: AliasAction,
    },
    /// Pane ↔ agent correlation shim (exec'd inside the new pane)
    #[command(name = "_attach")]
    Attach {
        /// Agent ID to bind to this pane
        agent_id: String,
        /// Command to exec after registering the pane (replaces this process)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// Internal state snapshot endpoint for the Zellij sidebar plugin
    #[command(name = "_plugin-state", hide = true)]
    PluginState,
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
    Show { id: String },
}

#[derive(Subcommand)]
enum AgentAction {
    /// Spawn an agent into a new Zellij pane
    Spawn {
        #[arg(long)]
        workspace: String,
        #[arg(long)]
        role: String,
        #[arg(long, default_value = "claude-code")]
        runtime: String,
        #[arg(long)]
        cwd: Option<PathBuf>,
        /// Command to run inside the pane (e.g. `-- claude code`)
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        command: Vec<String>,
    },
    /// List agents
    Ls {
        #[arg(long)]
        workspace: Option<String>,
    },
    /// Show agent details
    Show { id: String },
}

#[derive(Subcommand)]
enum SessionAction {
    /// Show daemon status
    Status,
}

#[derive(Subcommand)]
enum CaptureAction {
    /// Print a one-shot screen snapshot of an agent's pane
    Snapshot {
        /// Agent ID
        agent_id: String,
    },
    /// Stream pane output as ndjson (Ctrl-C to stop)
    Tail {
        /// Agent ID
        agent_id: String,
        /// Poll continuously at 5 Hz
        #[arg(long)]
        follow: bool,
    },
}

#[derive(Subcommand)]
enum AliasAction {
    /// Set an alias for a workspace
    Set {
        /// Alias name (e.g. "my-project")
        alias: String,
        /// Workspace ID to point to
        workspace_id: String,
    },
    /// Resolve an alias to its workspace ID
    Get {
        /// Alias name
        alias: String,
    },
    /// List all aliases
    Ls,
}

#[derive(Subcommand)]
enum LayoutAction {
    /// Print the bundled rally.kdl layout to stdout
    Export,
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
        Commands::Up { .. } => "up",
        Commands::Down { .. } => "down",
        Commands::Capture { action } => match action {
            CaptureAction::Snapshot { .. } => "capture snapshot",
            CaptureAction::Tail { .. } => "capture tail",
        },
        Commands::Alias { action } => match action {
            AliasAction::Set { .. } => "alias set",
            AliasAction::Get { .. } => "alias get",
            AliasAction::Ls => "alias ls",
        },
        Commands::InstallPlugin => "install-plugin",
        Commands::Layout { .. } => "layout export",
        Commands::Attach { .. } => "_attach",
        Commands::PluginState => "_plugin-state",
    };
    debug!(command = command_name, json = cli.json, "cli dispatched");

    match cli.command {
        Commands::Workspace { action } => {
            let mut client = connect(&socket_path).await?;
            match action {
                WorkspaceAction::New { name, repo } => {
                    let resp = client
                        .call(Request::CreateWorkspace {
                            name: name.into(),
                            repo,
                        })
                        .await?;
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
                AgentAction::Spawn {
                    workspace,
                    role,
                    runtime,
                    cwd,
                    command,
                } => {
                    let ws_id = parse_workspace_id(&workspace)?;
                    let resp = client
                        .call(Request::RegisterAgent {
                            workspace_id: ws_id,
                            role: role.clone().into(),
                            runtime: runtime.into(),
                        })
                        .await?;

                    // If a command was given, open a Zellij pane with the _attach shim
                    if !command.is_empty() {
                        if let Response::Agent(ref view) = resp {
                            let agent_id = view.id.to_string();
                            let session = rally_host_zellij::PluginBootstrap::detect();
                            let exe = std::env::current_exe()?;
                            let mut pane_cmd: Vec<String> = vec![
                                exe.to_string_lossy().into_owned(),
                                "_attach".into(),
                                agent_id.clone(),
                            ];
                            pane_cmd.extend(command);
                            let pane_cmd_refs: Vec<&str> =
                                pane_cmd.iter().map(|s| s.as_str()).collect();
                            info!(agent_id, "spawning agent pane via zellij");
                            rally_host_zellij::ZellijActions::new_pane(
                                session.as_ref(),
                                Some(&role),
                                cwd.as_deref(),
                                &pane_cmd_refs,
                            )?;
                        }
                    }
                    print_response(&resp, cli.json);
                }
                AgentAction::Ls { workspace } => {
                    let ws_id = workspace.as_deref().map(parse_workspace_id).transpose()?;
                    let resp = client
                        .call(Request::ListAgents {
                            workspace_id: ws_id,
                        })
                        .await?;
                    print_response(&resp, cli.json);
                }
                AgentAction::Show { id } => {
                    let agent_id = parse_agent_id(&id)?;
                    let resp = client.call(Request::GetAgent { id: agent_id }).await?;
                    print_response(&resp, cli.json);
                }
            }
        }

        Commands::Session { action } => match action {
            SessionAction::Status => {
                if socket_path.exists() {
                    let pid_path = socket_path.with_extension("pid");
                    let pid = std::fs::read_to_string(&pid_path).unwrap_or_default();
                    if cli.json {
                        println!(
                            r#"{{"status":"running","socket":"{}","pid":{}}}"#,
                            socket_path.display(),
                            pid.trim()
                        );
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
        },

        Commands::Up { workspace } => {
            let session_name = format!("rally-{workspace}");
            info!(session = %session_name, "rally up: spawning standalone session");
            // exec's into zellij — does not return on success
            rally_host_zellij::StandaloneBootstrap::up(&session_name, None)?;
        }

        Commands::Down { workspace } => {
            let session_name = format!("rally-{workspace}");
            info!(session = %session_name, "rally down: tearing down standalone session");
            rally_host_zellij::StandaloneBootstrap::down(&session_name)?;
            println!("stopped: {session_name}");
        }

        Commands::Alias { action } => {
            let mut client = connect(&socket_path).await?;
            match action {
                AliasAction::Set {
                    alias,
                    workspace_id,
                } => {
                    let ws_id = parse_workspace_id(&workspace_id)?;
                    let resp = client
                        .call(Request::SetAlias {
                            alias: alias.into(),
                            workspace_id: ws_id,
                        })
                        .await?;
                    print_response(&resp, cli.json);
                }
                AliasAction::Get { alias } => {
                    let resp = client
                        .call(Request::ResolveAlias {
                            alias: alias.into(),
                        })
                        .await?;
                    print_response(&resp, cli.json);
                }
                AliasAction::Ls => {
                    let resp = client.call(Request::ListAliases).await?;
                    print_response(&resp, cli.json);
                }
            }
        }

        Commands::PluginState => {
            let mut client = connect(&socket_path).await?;
            let resp = client.call(Request::GetStateSnapshot).await?;
            print_response(&resp, true);
        }

        Commands::Capture { action } => {
            let mut client = connect(&socket_path).await?;
            match action {
                CaptureAction::Snapshot { agent_id } => {
                    let aid = parse_agent_id(&agent_id)?;
                    let resp = client.call(Request::GetAgent { id: aid }).await?;
                    if let Response::Agent(view) = resp {
                        let numeric_pane = view.pane_id
                            .ok_or_else(|| anyhow::anyhow!("agent has no pane attached yet — run rally agent spawn with a command first"))?;
                        let session = view.pane_session.map(|s| rally_host_zellij::SessionHandle {
                            session_name: s,
                            detected_via: rally_host_zellij::DetectedVia::EnvVar,
                            owned: false,
                        });
                        let src = rally_capture::DumpScreenSource::new(session, numeric_pane);
                        let snap = src.snapshot()?;
                        if cli.json {
                            let out = serde_json::json!({
                                "pane_id": snap.pane_id,
                                "session": snap.session_name,
                                "text": snap.text,
                            });
                            println!("{out}");
                        } else {
                            print!("{}", snap.text);
                        }
                    } else {
                        print_response(&resp, cli.json);
                    }
                }
                CaptureAction::Tail { agent_id, follow } => {
                    let aid = parse_agent_id(&agent_id)?;
                    let resp = client.call(Request::GetAgent { id: aid }).await?;
                    if let Response::Agent(view) = resp {
                        let numeric_pane = view
                            .pane_id
                            .ok_or_else(|| anyhow::anyhow!("agent has no pane attached yet"))?;
                        let session = view.pane_session.map(|s| rally_host_zellij::SessionHandle {
                            session_name: s,
                            detected_via: rally_host_zellij::DetectedVia::EnvVar,
                            owned: false,
                        });
                        let src = rally_capture::DumpScreenSource::new(session, numeric_pane);
                        if follow {
                            src.tail_follow(std::time::Duration::from_millis(200))?;
                        } else {
                            let snap = src.snapshot()?;
                            print!("{}", snap.text);
                        }
                    } else {
                        print_response(&resp, cli.json);
                    }
                }
            }
        }

        Commands::InstallPlugin => {
            println!("Rally Zellij plugin (Phase 7 — not yet built).");
            println!();
            println!("For now, use plugin mode by starting rally commands from inside");
            println!("an existing Zellij session. The ZELLIJ_SESSION_NAME env is auto-detected.");
            println!();
            println!("To use standalone mode:");
            println!("  rally up <workspace-name>");
            println!("  zellij attach rally-<workspace-name>");
        }

        Commands::Layout { action } => match action {
            LayoutAction::Export => {
                print!("{}", BUNDLED_LAYOUT_KDL);
            }
        },

        Commands::Attach { agent_id, command } => {
            // This runs *inside* the new Zellij pane created by `agent spawn`.
            // 1. Read pane correlation from Zellij env
            // 2. Report to daemon via BindPane
            // 3. exec the real command (replaces this process)
            use rally_host_zellij::shim::{
                log_attach_correlation, log_attach_env_missing, PaneContext,
            };

            match PaneContext::from_env() {
                Ok(ctx) => {
                    log_attach_correlation(&agent_id, &ctx);
                    let aid = parse_agent_id(&agent_id)?;
                    let mut client = connect(&socket_path).await?;
                    let _ = client
                        .call(Request::BindPane {
                            agent_id: aid,
                            session_name: ctx.session_name,
                            tab_index: ctx.tab_index,
                            pane_id: ctx.pane_id,
                        })
                        .await?;
                }
                Err(e) => {
                    log_attach_env_missing(&e);
                    // Not inside Zellij — skip bind, still exec the command
                }
            }

            if command.is_empty() {
                // No command — drop into an interactive shell
                exec_command("zsh", &[])?;
            } else {
                exec_command(&command[0], &command[1..])?;
            }
        }
    }

    Ok(())
}

/// Replace the current process with `prog args` (Unix exec syscall).
fn exec_command(prog: &str, args: &[String]) -> anyhow::Result<()> {
    use std::os::unix::process::CommandExt;
    let mut cmd = std::process::Command::new(prog);
    cmd.args(args);
    let err = cmd.exec(); // only returns on error
    Err(anyhow::anyhow!("exec {prog} failed: {err}"))
}

async fn connect(socket_path: &std::path::Path) -> anyhow::Result<IpcClient> {
    debug!(socket = %socket_path.display(), "connecting to daemon");
    match IpcClient::connect(socket_path).await {
        Ok(c) => Ok(c),
        Err(_) => {
            eprintln!("rally: daemon not running, starting rallyd...");
            autostart_daemon(socket_path)?;
            IpcClient::connect(socket_path).await.inspect_err(|_| {
                eprintln!("rally: still cannot connect after autostart");
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

    let mut cmd = Command::new(&rallyd);
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .env("RALLY_DAEMON_SOCKET_PATH", socket_path);

    info!(rallyd_path = %rallyd.display(), socket = %socket_path.display(), "autostarting rallyd");
    let _child = cmd.spawn()?;

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
                if let Some(ref s) = a.pane_session {
                    println!("{:<12} {}", "pane:", s);
                }
            }
            Response::AgentList { items: list } => {
                if list.is_empty() {
                    println!("no agents");
                } else {
                    for a in list {
                        let pane = a.pane_session.as_deref().unwrap_or("-");
                        println!(
                            "{} {} {:?} pane:{} (ws:{})",
                            a.id, a.role, a.state, pane, a.workspace_id
                        );
                    }
                }
            }
            Response::AliasResolved { workspace_id } => {
                if let Some(id) = workspace_id {
                    println!("{id}");
                } else {
                    println!("alias not found");
                }
            }
            Response::AliasList { items } => {
                if items.is_empty() {
                    println!("no aliases");
                } else {
                    for a in items {
                        println!("{:<20} → {}", a.alias, a.workspace_id);
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
    let ulid: ulid::Ulid = s
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid workspace id '{s}': {e}"))?;
    Ok(rally_proto::v1::WorkspaceId::new(ulid))
}

fn parse_agent_id(s: &str) -> anyhow::Result<rally_proto::v1::AgentId> {
    let ulid: ulid::Ulid = s
        .parse()
        .map_err(|e| anyhow::anyhow!("invalid agent id '{s}': {e}"))?;
    Ok(rally_proto::v1::AgentId::new(ulid))
}

/// Bundled Zellij layout KDL for standalone mode (`rally up`).
const BUNDLED_LAYOUT_KDL: &str = r#"
layout {
    pane size=1 borderless=true {
        plugin location="zellij:tab-bar"
    }
    pane split_direction="vertical" {
        pane name="agents" size="80%"
        pane name="rally-sidebar" size="20%" {
            plugin location="file:~/.config/rally/rally.wasm" {
                // rally sidebar — install with: rally install-plugin
            }
        }
    }
    pane size=2 borderless=true {
        plugin location="zellij:status-bar"
    }
}
"#;
