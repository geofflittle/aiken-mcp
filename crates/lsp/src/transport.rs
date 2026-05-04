//! LSP wire transport — Content-Length-framed JSON-RPC over a child stdio.
//!
//! This module is intentionally LSP-types-agnostic at the framing layer; only
//! the request/notify methods touch lsp-types' marker types.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use lsp_types::request::Request as LspRequest;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{ChildStdin, Command};
use tokio::sync::{oneshot, Mutex};
use tokio::time::timeout;
use tracing::{debug, warn};

use aiken_mcp_core::{CoreError, CoreResult};

type PendingMap = Arc<Mutex<HashMap<i64, oneshot::Sender<Value>>>>;

#[derive(Clone)]
pub(crate) struct LspTransport {
    stdin: Arc<Mutex<ChildStdin>>,
    pending: PendingMap,
}

impl LspTransport {
    pub(crate) async fn spawn(binary: &Path, args: &[&str]) -> CoreResult<Self> {
        let mut cmd = Command::new(binary);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd.spawn().map_err(|err| match err.kind() {
            std::io::ErrorKind::NotFound => CoreError::AikenNotInstalled,
            _ => CoreError::Io(err),
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| CoreError::other("lsp child stdin missing"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| CoreError::other("lsp child stdout missing"))?;

        // Discard stderr to prevent pipe buffer fill blocking the LSP.
        if let Some(mut stderr) = child.stderr.take() {
            tokio::spawn(async move {
                let mut buf = [0u8; 4096];
                while let Ok(n) = stderr.read(&mut buf).await {
                    if n == 0 {
                        break;
                    }
                    debug!(target: "lsp::stderr", bytes = n);
                }
            });
        }

        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let pending_clone = pending.clone();
        tokio::spawn(reader_task(BufReader::new(stdout), pending_clone));

        // Detach child; we never wait on it explicitly. When transport drops
        // and stdin closes, the LSP server should exit on its own.
        tokio::spawn(async move {
            let _ = child.wait().await;
        });

        Ok(Self {
            stdin: Arc::new(Mutex::new(stdin)),
            pending,
        })
    }

    pub(crate) async fn request<R: LspRequest>(
        &self,
        id: i64,
        params: R::Params,
        wait: Duration,
    ) -> CoreResult<R::Result>
    where
        R::Params: Serialize,
        R::Result: DeserializeOwned,
    {
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);

        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": R::METHOD,
            "params": params,
        });
        write_message(&self.stdin, &payload).await?;

        let value = match timeout(wait, rx).await {
            Ok(Ok(v)) => v,
            Ok(Err(_)) => return Err(CoreError::other("lsp response channel closed")),
            Err(_) => {
                self.pending.lock().await.remove(&id);
                return Err(CoreError::other(format!(
                    "lsp request timed out: method={}",
                    R::METHOD
                )));
            }
        };

        if let Some(err) = value.get("error") {
            return Err(CoreError::other(format!("lsp error: {err}")));
        }

        let result_field = value
            .get("result")
            .cloned()
            .unwrap_or(Value::Null);
        serde_json::from_value::<R::Result>(result_field).map_err(CoreError::Serde)
    }

    pub(crate) async fn notify<P: Serialize>(
        &self,
        method: &str,
        params: P,
    ) -> CoreResult<()> {
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        write_message(&self.stdin, &payload).await
    }
}

async fn write_message(
    stdin: &Arc<Mutex<ChildStdin>>,
    payload: &Value,
) -> CoreResult<()> {
    let body = serde_json::to_vec(payload).map_err(CoreError::Serde)?;
    let mut guard = stdin.lock().await;
    let header = format!("Content-Length: {}\r\n\r\n", body.len());
    guard
        .write_all(header.as_bytes())
        .await
        .map_err(CoreError::Io)?;
    guard.write_all(&body).await.map_err(CoreError::Io)?;
    guard.flush().await.map_err(CoreError::Io)?;
    Ok(())
}

async fn reader_task<R: tokio::io::AsyncRead + Unpin>(
    mut reader: BufReader<R>,
    pending: PendingMap,
) {
    loop {
        let body = match read_message(&mut reader).await {
            Ok(Some(b)) => b,
            Ok(None) => return,
            Err(e) => {
                warn!(error = %e, "lsp reader error, exiting");
                return;
            }
        };

        let value: Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => {
                warn!(error = %e, "lsp message parse failure");
                continue;
            }
        };

        if let Some(id) = value.get("id").and_then(|v| v.as_i64()) {
            let tx_opt = pending.lock().await.remove(&id);
            if let Some(tx) = tx_opt {
                let _ = tx.send(value);
            }
        }
    }
}

async fn read_message<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut BufReader<R>,
) -> CoreResult<Option<Vec<u8>>> {
    use tokio::io::AsyncBufReadExt;

    let mut content_length: Option<usize> = None;
    let mut header_line = String::new();
    loop {
        header_line.clear();
        let n = reader.read_line(&mut header_line).await.map_err(CoreError::Io)?;
        if n == 0 {
            return Ok(None);
        }
        let trimmed = header_line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(rest) = trimmed.strip_prefix("Content-Length:") {
            content_length = rest.trim().parse().ok();
        }
    }

    let len = content_length.ok_or_else(|| CoreError::other("missing Content-Length header"))?;
    let mut buf = vec![0u8; len];
    reader
        .read_exact(&mut buf)
        .await
        .map_err(CoreError::Io)?;
    Ok(Some(buf))
}
