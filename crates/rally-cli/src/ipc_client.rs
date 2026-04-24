use std::path::Path;
use std::time::Duration;

use rally_proto::v1::{Request, RequestEnvelope, Response, ResponseEnvelope};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tracing::debug;

const CLIENT_TIMEOUT: Duration = Duration::from_secs(5);

pub struct IpcClient {
    stream: BufReader<UnixStream>,
}

impl IpcClient {
    pub async fn connect(socket_path: &Path) -> anyhow::Result<Self> {
        debug!(socket = %socket_path.display(), "connecting to daemon");
        let stream = UnixStream::connect(socket_path).await?;
        Ok(Self {
            stream: BufReader::new(stream),
        })
    }

    pub async fn call(&mut self, request: Request) -> anyhow::Result<Response> {
        let envelope = RequestEnvelope {
            request_id: compact_str::CompactString::from(ulid::Ulid::new().to_string()),
            client_pid: Some(std::process::id()),
            payload: request,
        };

        let mut line = serde_json::to_string(&envelope)?;
        line.push('\n');

        let stream = self.stream.get_mut();
        stream.write_all(line.as_bytes()).await?;
        stream.flush().await?;

        let mut resp_line = String::new();
        tokio::time::timeout(CLIENT_TIMEOUT, self.stream.read_line(&mut resp_line))
            .await
            .map_err(|_| anyhow::anyhow!("IPC call timed out after 5s"))??;

        let resp_env: ResponseEnvelope = serde_json::from_str(&resp_line)?;
        Ok(resp_env.payload)
    }
}
