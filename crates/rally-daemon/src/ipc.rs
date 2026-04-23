use std::sync::Arc;

use rally_proto::v1::{Request, RequestEnvelope, Response, ResponseEnvelope};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tracing::{debug, error, info, info_span, warn, Instrument};

use crate::services::RallyService;

/// Run the IPC server loop. Accepts connections on the unix socket and
/// dispatches each to a per-connection handler.
pub async fn serve(listener: UnixListener, service: Arc<RallyService>) {
    info!("IPC server accepting connections");
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let svc = Arc::clone(&service);
                tokio::spawn(async move {
                    debug!("connection accepted");
                    if let Err(e) = handle_connection(stream, svc).await {
                        warn!(error = %e, "connection handler error");
                    }
                });
            }
            Err(e) => {
                error!(error = %e, "accept error");
            }
        }
    }
}

/// Handle a single client connection. Reads newline-delimited JSON requests,
/// dispatches to the service, writes newline-delimited JSON responses.
async fn handle_connection(
    stream: UnixStream,
    service: Arc<RallyService>,
) -> anyhow::Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let envelope: RequestEnvelope = match serde_json::from_str(&line) {
            Ok(env) => env,
            Err(e) => {
                warn!(error = %e, "failed to parse request");
                let err_resp = ResponseEnvelope {
                    request_id: compact_str::CompactString::from("unknown"),
                    payload: Response::Error { message: format!("parse error: {e}") },
                };
                let mut out = serde_json::to_string(&err_resp)?;
                out.push('\n');
                writer.write_all(out.as_bytes()).await?;
                continue;
            }
        };

        let request_id = envelope.request_id.clone();
        let method = method_name(&envelope.payload);
        let client_pid = envelope.client_pid;

        let span = info_span!(
            "ipc_request",
            %request_id,
            method,
            client_pid = client_pid.unwrap_or(0),
        );

        let started = std::time::Instant::now();
        let response = async {
            debug!("request_in");
            let result = dispatch(&service, envelope.payload);
            let duration_ms = started.elapsed().as_millis() as u64;
            match &result {
                Ok(_) => debug!(duration_ms, "response_out"),
                Err(e) => warn!(duration_ms, error = %e, "request failed"),
            }
            result
        }
        .instrument(span)
        .await;

        let payload = match response {
            Ok(r) => r,
            Err(e) => Response::Error { message: e.to_string() },
        };

        let resp_envelope = ResponseEnvelope {
            request_id,
            payload,
        };
        let mut out = serde_json::to_string(&resp_envelope)?;
        out.push('\n');
        writer.write_all(out.as_bytes()).await?;
    }

    Ok(())
}

fn dispatch(
    service: &RallyService,
    request: Request,
) -> anyhow::Result<Response> {
    match request {
        Request::CreateWorkspace { name, repo } => {
            let view = service.create_workspace(name, repo)?;
            Ok(Response::Workspace(view))
        }
        Request::ListWorkspaces => {
            let list = service.list_workspaces()?;
            Ok(Response::WorkspaceList { items: list })
        }
        Request::GetWorkspace { id } => {
            match service.get_workspace(id)? {
                Some(ws) => Ok(Response::Workspace(ws)),
                None => Ok(Response::Error { message: format!("workspace {id} not found") }),
            }
        }
        Request::RegisterAgent { workspace_id, role, runtime } => {
            let view = service.register_agent(workspace_id, role, runtime)?;
            Ok(Response::Agent(view))
        }
        Request::GetAgent { id } => {
            match service.get_agent(id)? {
                Some(a) => Ok(Response::Agent(a)),
                None => Ok(Response::Error { message: format!("agent {id} not found") }),
            }
        }
        Request::ListAgents { workspace_id } => {
            let list = service.list_agents(workspace_id)?;
            Ok(Response::AgentList { items: list })
        }
        Request::ArchiveWorkspace { .. } => {
            Ok(Response::Error { message: "archive not yet implemented".into() })
        }
        Request::EmitAgentEvent { .. } => {
            Ok(Response::Error { message: "emit not yet implemented".into() })
        }
        Request::ListInbox { .. } => {
            Ok(Response::Error { message: "inbox not yet implemented".into() })
        }
        Request::AckInboxItem { .. } => {
            Ok(Response::Error { message: "ack not yet implemented".into() })
        }
        _ => {
            Ok(Response::Error { message: "unknown method".into() })
        }
    }
}

fn method_name(req: &Request) -> &'static str {
    match req {
        Request::CreateWorkspace { .. } => "create_workspace",
        Request::ArchiveWorkspace { .. } => "archive_workspace",
        Request::ListWorkspaces => "list_workspaces",
        Request::GetWorkspace { .. } => "get_workspace",
        Request::RegisterAgent { .. } => "register_agent",
        Request::GetAgent { .. } => "get_agent",
        Request::ListAgents { .. } => "list_agents",
        Request::EmitAgentEvent { .. } => "emit_agent_event",
        Request::ListInbox { .. } => "list_inbox",
        Request::AckInboxItem { .. } => "ack_inbox_item",
        _ => "unknown",
    }
}
