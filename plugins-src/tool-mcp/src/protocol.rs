//! MCP stdio 传输 + JSON-RPC 2.0 客户端（newline-delimited，无
//! Content-Length 帧——MCP stdio 规范按行分隔）。每个连接一个 stdout
//! 读取线程（recv_timeout 才能带超时等响应）；stderr 收进诊断缓冲；
//! Drop 杀子进程。

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use plugin_api::serde_json::{Value, json};

/// 一次请求的错误分类：传输断开（可懒重连）/ 超时 / 协议层报错（原样
/// 返回给模型）。决定 registry 是否重置连接。
#[derive(Debug)]
pub(crate) enum McpError {
    Transport(String),
    Timeout(String),
    Protocol(String),
}

impl McpError {
    pub(crate) fn is_transport(&self) -> bool {
        matches!(self, McpError::Transport(_))
    }
}

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpError::Transport(m) => write!(f, "{m}"),
            McpError::Timeout(m) => write!(f, "{m}"),
            McpError::Protocol(m) => write!(f, "{m}"),
        }
    }
}

pub(crate) struct McpConn {
    child: Child,
    stdin: ChildStdin,
    rx: Receiver<String>,
    stderr_buf: Arc<Mutex<Vec<u8>>>,
    next_id: u64,
}

impl McpConn {
    /// spawn 子进程并接管三根管道。env 为**增量**（叠加在宿主环境上）。
    pub(crate) fn spawn(
        command: &str,
        args: &[String],
        env: &BTreeMap<String, String>,
    ) -> Result<McpConn, String> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in env {
            cmd.env(k, v);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("mcp spawn `{command}`: {e}"))?;
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");
        let (tx, rx) = channel::<String>();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                match line {
                    Ok(l) => {
                        if tx.send(l).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        let stderr_buf = Arc::new(Mutex::new(Vec::new()));
        {
            let buf = Arc::clone(&stderr_buf);
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    let mut b = buf.lock().unwrap();
                    if b.len() < 8 * 1024 {
                        b.extend_from_slice(line.as_bytes());
                        b.push(b'\n');
                    }
                }
            });
        }
        let stdin = child.stdin.take().expect("stdin piped");
        Ok(McpConn {
            child,
            stdin,
            rx,
            stderr_buf,
            next_id: 0,
        })
    }

    /// 最近的 stderr 摘要（握手失败时的诊断信息）。
    pub(crate) fn stderr_tail(&self) -> String {
        let b = self.stderr_buf.lock().unwrap();
        String::from_utf8_lossy(&b).trim().to_string()
    }

    fn send(&mut self, msg: &Value) -> Result<(), McpError> {
        let mut line = msg.to_string();
        line.push('\n');
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.flush())
            .map_err(|e| McpError::Transport(format!("mcp write: {e}")))
    }

    /// 发通知（无 id、不等响应）。
    pub(crate) fn notify(&mut self, method: &str) -> Result<(), McpError> {
        self.send(&json!({ "jsonrpc": "2.0", "method": method }))
    }

    /// 发请求并等配 id 的响应；通知与失配响应跳过；超时按 deadline。
    pub(crate) fn request(
        &mut self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, McpError> {
        self.next_id += 1;
        let id = self.next_id;
        self.send(&json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }))?;
        let deadline = Instant::now() + timeout;
        loop {
            let remain = deadline
                .checked_duration_since(Instant::now())
                .ok_or_else(|| McpError::Timeout(format!("mcp {method}: timed out")))?;
            match self.rx.recv_timeout(remain) {
                Ok(line) => {
                    let Ok(v) = plugin_api::serde_json::from_str::<Value>(&line) else {
                        continue; // 非法行跳过
                    };
                    if v.get("id").and_then(Value::as_u64) != Some(id) {
                        continue; // notification / 他人响应
                    }
                    if let Some(err) = v.get("error") {
                        let msg = err
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown error");
                        return Err(McpError::Protocol(format!("mcp {method}: {msg}")));
                    }
                    return Ok(v.get("result").cloned().unwrap_or(Value::Null));
                }
                Err(RecvTimeoutError::Timeout) => {
                    return Err(McpError::Timeout(format!(
                        "mcp {method}: timed out after {timeout:?}"
                    )));
                }
                Err(RecvTimeoutError::Disconnected) => {
                    let tail = self.stderr_tail();
                    let detail = if tail.is_empty() {
                        "server closed the connection".to_string()
                    } else {
                        format!("server exited: {tail}")
                    };
                    return Err(McpError::Transport(format!("mcp {method}: {detail}")));
                }
            }
        }
    }

    /// MCP initialize 握手 + initialized 通知（serverInfo 供诊断）。
    pub(crate) fn initialize(&mut self, handshake: Duration) -> Result<Value, McpError> {
        let info = self.request(
            "initialize",
            json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "locowrk-tool-mcp", "version": "1.0.0" }
            }),
            handshake,
        )?;
        self.notify("notifications/initialized")?;
        Ok(info)
    }
}

impl Drop for McpConn {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
