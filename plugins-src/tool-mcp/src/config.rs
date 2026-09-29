//! mcp.json 解析与校验。位置：`<数据根>/mcp.json`（用户手写配置，模型
//! 不可达——本插件不提供任何改配置的工具，信任面收敛在用户）。
//!
//! 形态：
//! ```json
//! { "servers": { "<name>": {
//!       "command": "npx", "args": ["-y", "@modelcontextprotocol/server-x"],
//!       "env": { "KEY": "v" },
//!       "side_effect": "pure",              // 缺省 irreversible（§五 安全兜底）
//!       "tool_effects": { "safe_tool": "pure" },
//!       "timeout_ms": 30000 } } }
//! ```
//! 单个 server 配置坏 → 记为配置错误条目（mcp.status 可见），不拖垮整个
//! 插件（fail-soft）；整个文件坏 → 一条 `config` 错误、零 server。

use std::path::PathBuf;

use plugin_api::fsutil::data_root;
use plugin_api::serde_json::{Map, Value};

/// 远端工具没有机械 undo（undo_tool 无从谈起）：`compensable` 按宁严勿松
/// 收紧为 Irreversible，`pure` 是用户显式信任声明，其余值非法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Grade {
    Pure,
    Irreversible,
}

impl Grade {
    fn parse(s: &str) -> Option<Grade> {
        match s.to_ascii_lowercase().as_str() {
            "pure" => Some(Grade::Pure),
            "irreversible" | "compensable" => Some(Grade::Irreversible),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ServerCfg {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: std::collections::BTreeMap<String, String>,
    /// 该 server 全部工具的缺省级别（tool_effects 可逐个覆盖）。
    pub default_grade: Grade,
    pub tool_effects: std::collections::BTreeMap<String, Grade>,
    pub timeout_ms: u64,
}

/// 单 server 配置。
struct ServerSpec {
    name: String,
    command: String,
    args: Vec<String>,
    env: std::collections::BTreeMap<String, String>,
    default_grade: Grade,
    tool_effects: std::collections::BTreeMap<String, Grade>,
    timeout_ms: u64,
}

impl ServerSpec {
    fn into_cfg(self) -> ServerCfg {
        ServerCfg {
            name: self.name,
            command: self.command,
            args: self.args,
            env: self.env,
            default_grade: self.default_grade,
            tool_effects: self.tool_effects,
            timeout_ms: self.timeout_ms,
        }
    }
}

pub(crate) const TIMEOUT_DEFAULT_MS: u64 = 30_000;
pub(crate) const TIMEOUT_MAX_MS: u64 = 120_000;
/// 握手超时（连接建立 + initialize + tools/list 的总预算）。
pub(crate) const HANDSHAKE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);

fn config_path() -> Result<PathBuf, String> {
    Ok(data_root()?.join("mcp.json"))
}

/// server 名约束：小写字母开头，`[a-z0-9_-]`，≤32 字符——本地工具名是
/// `mcp.<server>.<tool>`，名字里不能出现 `.`（路由歧义）也不能为空。
fn valid_server_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && name.len() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// 工具名清洗：MCP 工具名可能是任意字符串；统一替换非法字符，
/// 清洗后为空/纯下划线的工具跳过。
pub(crate) fn sanitize_tool_name(raw: &str) -> Option<String> {
    let s: String = raw
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if s.is_empty() || s.chars().all(|c| c == '_') {
        None
    } else {
        Some(s)
    }
}

/// 读 + 解析 mcp.json。返回（合法配置按名字排序, 配置错误条目）。
pub(crate) fn load() -> (Vec<ServerCfg>, Vec<(String, String)>) {
    let mut cfgs = Vec::new();
    let mut errors = Vec::new();

    let path = match config_path() {
        Ok(p) => p,
        Err(e) => {
            errors.push(("config".to_string(), e));
            return (cfgs, errors);
        }
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // 无配置 = 插件加载但不桥接任何 server（合法状态，非错误）
            return (cfgs, errors);
        }
        Err(e) => {
            errors.push((
                "config".to_string(),
                format!("read {}: {e}", path.display()),
            ));
            return (cfgs, errors);
        }
    };
    let parsed: Value = match serde_json_parse(&text) {
        Ok(v) => v,
        Err(e) => {
            errors.push(("config".to_string(), format!("parse: {e}")));
            return (cfgs, errors);
        }
    };
    let Some(servers) = parsed.get("servers").and_then(Value::as_object) else {
        errors.push((
            "config".to_string(),
            "missing object field `servers`".to_string(),
        ));
        return (cfgs, errors);
    };
    for (name, body) in servers {
        match parse_server(name, body) {
            Ok(spec) => cfgs.push(spec.into_cfg()),
            Err(e) => errors.push((name.clone(), e)),
        }
    }
    cfgs.sort_by(|a, b| a.name.cmp(&b.name));
    (cfgs, errors)
}

fn serde_json_parse(text: &str) -> Result<Value, String> {
    plugin_api::serde_json::from_str(text).map_err(|e| e.to_string())
}

fn parse_server(name: &str, body: &Value) -> Result<ServerSpec, String> {
    if !valid_server_name(name) {
        return Err("server name must match [a-z][a-z0-9_-]{0,31} (no dots)".to_string());
    }
    let obj: &Map<String, Value> = body
        .as_object()
        .ok_or_else(|| "server entry must be an object".to_string())?;
    let command = obj
        .get("command")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "missing string field `command`".to_string())?;
    if command.trim().is_empty() {
        return Err("`command` must not be empty".to_string());
    }
    let args = match obj.get("args") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(a)) => {
            let mut out = Vec::new();
            for v in a {
                out.push(
                    v.as_str()
                        .ok_or_else(|| "`args` must be an array of strings".to_string())?
                        .to_string(),
                );
            }
            out
        }
        Some(_) => return Err("`args` must be an array of strings".to_string()),
    };
    let env = match obj.get("env") {
        None | Some(Value::Null) => Default::default(),
        Some(Value::Object(m)) => {
            let mut out = std::collections::BTreeMap::new();
            for (k, v) in m {
                let v = v
                    .as_str()
                    .ok_or_else(|| format!("`env.{k}` must be a string"))?;
                out.insert(k.clone(), v.to_string());
            }
            out
        }
        Some(_) => return Err("`env` must be an object of strings".to_string()),
    };
    let default_grade = match obj.get("side_effect") {
        None | Some(Value::Null) => Grade::Irreversible,
        Some(Value::String(s)) => Grade::parse(s).ok_or_else(|| {
            format!("unsupported `side_effect` `{s}` (pure|irreversible|compensable)")
        })?,
        Some(_) => return Err("`side_effect` must be a string".to_string()),
    };
    let tool_effects = match obj.get("tool_effects") {
        None | Some(Value::Null) => Default::default(),
        Some(Value::Object(m)) => {
            let mut out = std::collections::BTreeMap::new();
            for (k, v) in m {
                let s = v
                    .as_str()
                    .ok_or_else(|| format!("`tool_effects.{k}` must be a string"))?;
                let g = Grade::parse(s).ok_or_else(|| {
                    format!("unsupported `tool_effects.{k}` `{s}` (pure|irreversible|compensable)")
                })?;
                out.insert(k.clone(), g);
            }
            out
        }
        Some(_) => return Err("`tool_effects` must be an object".to_string()),
    };
    let timeout_ms = match obj.get("timeout_ms") {
        None | Some(Value::Null) => TIMEOUT_DEFAULT_MS,
        Some(v) => {
            let n = v
                .as_u64()
                .ok_or_else(|| "`timeout_ms` must be a non-negative number".to_string())?;
            n.clamp(100, TIMEOUT_MAX_MS)
        }
    };
    Ok(ServerSpec {
        name: name.to_string(),
        command,
        args,
        env,
        default_grade,
        tool_effects,
        timeout_ms,
    })
}
