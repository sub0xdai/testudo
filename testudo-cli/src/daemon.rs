// @anchor infra:cli:daemon
// @tags infra

//! Daemon mode — background agent with Unix socket control.
//!
//! The daemon stays foreground (no fork) but writes a PID file and opens
//! a Unix domain socket for JSON-RPC control commands. Users can background
//! the process with `nohup`, `systemd`, or `screen`.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::net::UnixStream;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// State published by the daemon via watch channel and status RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonState {
    pub phase: String,
    pub signal_count: u64,
    pub uptime_secs: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl Default for DaemonState {
    fn default() -> Self {
        Self {
            phase: "Idle".into(),
            signal_count: 0,
            uptime_secs: 0,
            last_error: None,
        }
    }
}

/// Resolve the daemon's control files directory.
pub fn daemon_dir() -> PathBuf {
    crate::config::Config::config_dir()
}

/// Path to the PID file.
pub fn pid_path() -> PathBuf {
    daemon_dir().join("testudo.pid")
}

/// Path to the Unix domain socket.
pub fn socket_path() -> PathBuf {
    daemon_dir().join("testudo.sock")
}

/// Write the current process ID to the PID file.
pub fn write_pid_file() -> std::io::Result<()> {
    let dir = daemon_dir();
    std::fs::create_dir_all(&dir)?;
    let pid = std::process::id().to_string();
    std::fs::write(pid_path(), pid)
}

/// Remove the PID file (on shutdown).
pub fn remove_pid_file() {
    let _ = std::fs::remove_file(pid_path());
}

/// Remove the Unix socket file (on shutdown).
pub fn remove_socket() {
    let _ = std::fs::remove_file(socket_path());
}

/// Print daemon startup info to stdout so the user knows what's happening.
pub fn print_startup_info() {
    println!("Daemon started.");
    println!("  PID file: {}", pid_path().display());
    println!("  Socket:   {}", socket_path().display());
    println!("  Logs:     {}/logs/", daemon_dir().display());
    println!();
    println!("Control commands:");
    println!("  echo '{{\"method\":\"status\"}}' | nc -U {}", socket_path().display());
    println!("  echo '{{\"method\":\"stop\"}}'   | nc -U {}", socket_path().display());
    println!("  testudo attach");
}

/// Handle a single JSON-RPC control connection.
pub async fn handle_control_connection(
    stream: UnixStream,
    state: tokio::sync::watch::Receiver<DaemonState>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let req: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => {
                let err = r#"{"jsonrpc":"2.0","error":{"code":-32700,"message":"Parse error"}}"#;
                let _ = writer.write_all(err.as_bytes()).await;
                continue;
            }
        };

        let method = req["method"].as_str().unwrap_or("");
        let id = req.get("id").cloned().unwrap_or(serde_json::Value::Null);

        let response = match method {
            "status" => {
                let s = state.borrow().clone();
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": s,
                })
            }
            "ping" => {
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": "pong",
                })
            }
            "stop" => {
                tracing::info!("Stop command received via socket");
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": "stopping",
                });
                let _ = writer
                    .write_all(resp.to_string().as_bytes())
                    .await;
                let _ = writer.write_all(b"\n").await;
                // Signal shutdown — the caller should exit the accept loop
                std::process::exit(0);
            }
            _ => {
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("Method not found: {}", method),
                    }
                })
            }
        };

        let mut resp_str = response.to_string();
        resp_str.push('\n');
        let _ = writer.write_all(resp_str.as_bytes()).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn daemon_state_serialization_roundtrip() {
        let state = DaemonState {
            phase: "Observing".into(),
            signal_count: 5,
            uptime_secs: 120,
            last_error: None,
        };
        let json = serde_json::to_string(&state).unwrap();
        let restored: DaemonState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.phase, "Observing");
        assert_eq!(restored.signal_count, 5);
    }

    #[test]
    fn pid_path_has_correct_suffix() {
        let path = pid_path();
        assert!(path.ends_with("testudo.pid"));
    }

    #[test]
    fn socket_path_has_correct_suffix() {
        let path = socket_path();
        assert!(path.ends_with("testudo.sock"));
    }
}
