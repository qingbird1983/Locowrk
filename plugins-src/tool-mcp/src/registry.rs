//! 连接注册表：加载时发现（spawn → initialize → tools/list → 生成
//! ToolSpec 缓存），调用时按 server 懒重连。工具清单**冻结于加载时**
//!（describe 生成的 manifest 不随会话变），reload 只重连、不增删工具。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use plugin_api::serde_json::{Value, json};
use plugin_api::tool::{SideEffect, ToolSpec};

use crate::config::{self, Grade, ServerCfg};
use crate::protocol::McpConn;

/// 单个 MCP 工具的本地投影（describe 时冻结）。
#[derive(Debug, Clone)]
pub(crate) struct LocalTool {
    /// 清洗后的名字（`mcp_<server>_<tool>` 全名的尾段，非空）。
    pub local: String,
    /// MCP server 侧原始名（tools/call 用）。
    pub remote: String,
    pub description: String,
    pub schema: Value,
    pub grade: Grade,
}

pub(crate) struct ServerEntry {
    pub cfg: ServerCfg,
    pub conn: Option<McpConn>,
    /// 最近一次连接 / 握手 / 调用传输错误（mcp_status 可见）。
    pub error: Option<String>,
    pub tools: Vec<LocalTool>,
}

pub(crate) struct Registry {
    pub servers: Mutex<HashMap<String, Arc<Mutex<ServerEntry>>>>,
    /// 配置级错误（名字非法 / 字段缺失等），加载时固化。
    pub cfg_errors: Vec<(String, String)>,
}

/// describe 与 make_state 之间的共享点：describe 先发现并 stash，
/// make_state 接管。static 存活于插件进程内，测试用 [`reset`] 清场。
static REGISTRY: Mutex<Option<Arc<Registry>>> = Mutex::new(None);

pub(crate) fn take_or_discover() -> Arc<Registry> {
    let mut slot = REGISTRY.lock().unwrap();
    slot.take().unwrap_or_else(Registry::discover)
}

pub(crate) fn stash(registry: Arc<Registry>) {
    *REGISTRY.lock().unwrap() = Some(registry);
}

#[cfg(test)]
pub(crate) fn reset() {
    *REGISTRY.lock().unwrap() = None;
}

impl Registry {
    /// 全量发现：spawn + 握手 + 列工具。单个 server 失败记 error 不拖垮
    /// 其他 server（fail-soft）。
    pub(crate) fn discover() -> Arc<Registry> {
        let (cfgs, errors) = config::load();
        let registry = Registry {
            servers: Mutex::new(HashMap::new()),
            cfg_errors: errors,
        };
        for cfg in cfgs {
            let entry = Arc::new(Mutex::new(ServerEntry {
                cfg,
                conn: None,
                error: None,
                tools: Vec::new(),
            }));
            registry
                .servers
                .lock()
                .unwrap()
                .insert(entry.lock().unwrap().cfg.name.clone(), Arc::clone(&entry));
            if let Err(e) = registry.connect_and_list(&entry) {
                entry.lock().unwrap().error = Some(e);
            }
        }
        Arc::new(registry)
    }

    /// spawn + initialize + tools/list，成功则填 conn 与 tools 缓存。
    fn connect_and_list(&self, entry: &Arc<Mutex<ServerEntry>>) -> Result<(), String> {
        let (command, args, env) = {
            let e = entry.lock().unwrap();
            (e.cfg.command.clone(), e.cfg.args.clone(), e.cfg.env.clone())
        };
        let mut conn = McpConn::spawn(&command, &args, &env)?;
        conn.initialize(config::HANDSHAKE_TIMEOUT)
            .map_err(|e| e.to_string())?;
        let listing = conn
            .request("tools/list", json!({}), config::HANDSHAKE_TIMEOUT)
            .map_err(|e| e.to_string())?;
        let cfg = entry.lock().unwrap().cfg.clone();
        let tools = parse_tools(&cfg, &listing);
        let mut e = entry.lock().unwrap();
        e.conn = Some(conn);
        e.tools = tools;
        e.error = None;
        Ok(())
    }

    /// mcp_reload：全部断开重连（工具缓存保持加载时冻结的版本）。
    pub(crate) fn reload(&self) {
        let entries: Vec<Arc<Mutex<ServerEntry>>> =
            self.servers.lock().unwrap().values().cloned().collect();
        for entry in entries {
            entry.lock().unwrap().conn = None; // Drop 杀旧子进程
            if let Err(e) = self.connect_and_list(&entry) {
                entry.lock().unwrap().error = Some(e);
            } else {
                entry.lock().unwrap().error = None;
            }
        }
    }

    /// mcp_status：连接态 / 工具数 / 最近错误。
    pub(crate) fn status(&self) -> Vec<Value> {
        let mut rows = Vec::new();
        let mut names: Vec<String> = self.servers.lock().unwrap().keys().cloned().collect();
        names.sort();
        for name in names {
            let entry = Arc::clone(self.servers.lock().unwrap().get(&name).unwrap());
            let e = entry.lock().unwrap();
            rows.push(json!({
                "name": name,
                "command": e.cfg.command,
                "connected": e.conn.is_some(),
                "tools": e.tools.len(),
                "error": e.error,
            }));
        }
        rows
    }

    /// describe 用：管理工具之外的全部动态规格（按名排序，确定性）。
    pub(crate) fn tool_specs(&self) -> Vec<ToolSpec> {
        let mut names: Vec<String> = self.servers.lock().unwrap().keys().cloned().collect();
        names.sort();
        let mut specs = Vec::new();
        for name in names {
            let entry = Arc::clone(self.servers.lock().unwrap().get(&name).unwrap());
            let e = entry.lock().unwrap();
            for tool in &e.tools {
                let side_effect = match tool.grade {
                    Grade::Pure => SideEffect::Pure,
                    Grade::Irreversible => SideEffect::Irreversible,
                };
                specs.push(ToolSpec {
                    // server 名已由配置校验限定 [a-z0-9-]（无下划线），
                    // 全名 `mcp_<server>_<local>` 按首个下划线拆回无歧义
                    name: format!("mcp_{name}_{}", tool.local),
                    description: tool.description.clone(),
                    schema: tool.schema.clone(),
                    side_effect,
                    exposure: Default::default(),
                });
            }
        }
        specs
    }

    /// 工具调用路由：server 死连接懒重连（只握手不重列工具——清单冻结）。
    /// 工具级错误（isError）按 tool-fetch 惯例作为**数据**返回。
    pub(crate) fn call(&self, server: &str, tool: &str, args: Value) -> Result<Value, String> {
        let entry = Arc::clone(
            self.servers
                .lock()
                .unwrap()
                .get(server)
                .ok_or_else(|| format!("mcp: unknown server `{server}`"))?,
        );
        let mut e = entry.lock().unwrap();
        let remote = e
            .tools
            .iter()
            .find(|t| t.local == tool)
            .map(|t| t.remote.clone())
            .ok_or_else(|| format!("mcp: server `{server}` has no tool `{tool}`"))?;
        if e.conn.is_none() {
            let (command, args_cfg, env) =
                (e.cfg.command.clone(), e.cfg.args.clone(), e.cfg.env.clone());
            match McpConn::spawn(&command, &args_cfg, &env).and_then(|mut c| {
                c.initialize(config::HANDSHAKE_TIMEOUT)
                    .map(|_| c)
                    .map_err(|e| e.to_string())
            }) {
                Ok(c) => e.conn = Some(c),
                Err(err) => {
                    e.error = Some(err.clone());
                    return Err(format!("mcp: reconnect `{server}`: {err}"));
                }
            }
        }
        let timeout = Duration::from_millis(e.cfg.timeout_ms);
        let conn = e.conn.as_mut().expect("conn just ensured");
        match conn.request(
            "tools/call",
            json!({ "name": remote, "arguments": args }),
            timeout,
        ) {
            Ok(result) => Ok(extract_result(&result)),
            Err(err) => {
                // 传输断开 → 丢连接，下次调用懒重连；超时/协议错误保留现场
                if err.is_transport() {
                    e.conn = None;
                    e.error = Some(err.to_string());
                }
                Err(err.to_string())
            }
        }
    }

    /// 停掉全部子进程（Drop / 手工重连前）。
    fn kill_all(&self) {
        for entry in self.servers.lock().unwrap().values() {
            entry.lock().unwrap().conn = None;
        }
    }
}

impl Drop for Registry {
    fn drop(&mut self) {
        self.kill_all();
    }
}

/// tools/list 结果 → 本地投影（描述缺省、schema 缺省空对象、清洗后为空
/// 的工具跳过）。
fn parse_tools(cfg: &ServerCfg, listing: &Value) -> Vec<LocalTool> {
    let mut tools = Vec::new();
    let Some(list) = listing.get("tools").and_then(Value::as_array) else {
        return tools;
    };
    for t in list {
        let Some(remote) = t.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(local) = config::sanitize_tool_name(remote) else {
            continue;
        };
        let description = t
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("(no description provided by the MCP server)")
            .to_string();
        let schema = t
            .get("inputSchema")
            .cloned()
            .unwrap_or_else(|| json!({ "type": "object" }));
        let grade = cfg
            .tool_effects
            .get(remote)
            .or_else(|| cfg.tool_effects.get(&local))
            .copied()
            .unwrap_or(cfg.default_grade);
        tools.push(LocalTool {
            local,
            remote: remote.to_string(),
            description,
            schema,
            grade,
        });
    }
    tools
}

/// tools/call 结果投影：text 部件拼接；非 text 部件计数说明；isError 是
/// 数据不是传输错误。内容上限 1MiB（UTF-8 边界安全截断）。
fn extract_result(result: &Value) -> Value {
    const CAP: usize = 1024 * 1024;
    let mut parts: Vec<String> = Vec::new();
    let mut non_text = 0u64;
    if let Some(list) = result.get("content").and_then(Value::as_array) {
        for part in list {
            if part.get("type").and_then(Value::as_str) == Some("text") {
                if let Some(t) = part.get("text").and_then(Value::as_str) {
                    parts.push(t.to_string());
                }
            } else {
                non_text += 1;
            }
        }
    }
    let mut content = parts.join("\n");
    if non_text > 0 {
        content.push_str(&format!("\n[{non_text} non-text content part(s) omitted]"));
    }
    if content.len() > CAP {
        let mut end = CAP;
        while !content.is_char_boundary(end) {
            end -= 1;
        }
        content.truncate(end);
        content.push_str("\n[truncated]");
    }
    json!({
        "content": content,
        "is_error": result.get("isError").and_then(Value::as_bool).unwrap_or(false),
    })
}
