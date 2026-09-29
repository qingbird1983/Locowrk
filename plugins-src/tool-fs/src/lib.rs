//! tool-fs：第一批真实工具插件（plugins.md P0，阶段八③落地）。
//!
//! 只读先行——三个工具全部 `SideEffect::Pure`（plugins.md：
//! fs.read / fs.list / fs.grep → Pure，随便调，回退免费）：
//! - `fs.read`：读 UTF-8 文本文件，超过 max_bytes 截断（保护模型上下文）
//! - `fs.list`：列目录（目录在前，含类型与大小）
//! - `fs.grep`：大小写不敏感子串搜索；目录递归有深度与结果数上限
//!
//! fs.write（Compensable，写前备份）/ fs.delete（Irreversible，走审批）
//! 按排期在第二批；沙箱化（受限根目录）归 tool-sandbox（P2）。当前
//! 约定传绝对路径——相对路径按宿主进程 CWD 解析，工具描述里已说明。
//!
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。

use std::ffi::CString;
use std::io::Read;
use std::path::PathBuf;

use plugin_api::ffi::log_level;
use plugin_api::manifest::{PluginKind, PluginManifest};
use plugin_api::serde_json::{Value, json};
use plugin_api::tool::{SideEffect, ToolSpec};
use plugin_api::{HostVtable, PluginState};

const PLUGIN_ID: &str = "tool-fs";

/// 单文件读取缺省/上限（字节）。上限防一次调用灌爆模型上下文。
const READ_DEFAULT: u64 = 32 * 1024;
const READ_MAX: u64 = 1024 * 1024;
/// grep 默认/上限：结果条数、递归深度、扫描文件数、单行摘录长度。
const GREP_RESULTS_DEFAULT: u64 = 50;
const GREP_RESULTS_MAX: u64 = 200;
const GREP_DEPTH_DEFAULT: u64 = 3;
const GREP_DEPTH_MAX: u64 = 8;
const GREP_FILE_MAX: u64 = 1024 * 1024;
const GREP_LINE_EXCERPT: usize = 200;

fn plugin_version() -> &'static str {
    option_env!("LOCOWRK_PLUGIN_VERSION").unwrap_or("0.0.0-dev")
}

/// describe 符号的数据源：manifest JSON（NUL 结尾，约定见 plugin-api::ffi）。
fn describe_manifest() -> CString {
    let manifest = PluginManifest {
        id: PLUGIN_ID.into(),
        version: plugin_version().into(),
        abi_version: plugin_api::PLUGIN_ABI_VERSION,
        kind: PluginKind::Tool,
        tools: vec![
            ToolSpec {
                name: "fs.read".into(),
                description: "Read a UTF-8 text file from the local filesystem. \
                              Pass an absolute path. Returns {path, size, truncated, content}; \
                              content is capped at max_bytes (default 32KiB)."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "absolute file path" },
                        "max_bytes": { "type": "number",
                                       "description": "cap returned content (default 32768, max 1048576)" }
                    },
                    "required": ["path"]
                }),
                side_effect: SideEffect::Pure,
            },
            ToolSpec {
                name: "fs.list".into(),
                description: "List a directory. Pass an absolute path. \
                              Returns {path, entries: [{name, kind: file|dir|other, size}]}; \
                              directories come first, then names sorted."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "absolute directory path" }
                    },
                    "required": ["path"]
                }),
                side_effect: SideEffect::Pure,
            },
            ToolSpec {
                name: "fs.grep".into(),
                description: "Case-insensitive substring search in a text file, or \
                              recursively under a directory. Pass an absolute path. \
                              Returns {query, matches: [{file, line, text}], truncated, scanned}."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "absolute file or directory path" },
                        "query": { "type": "string", "description": "case-insensitive substring" },
                        "max_results": { "type": "number", "description": "default 50, max 200" },
                        "max_depth": { "type": "number", "description": "recursion depth, default 3, max 8" }
                    },
                    "required": ["path", "query"]
                }),
                side_effect: SideEffect::Pure,
            },
        ],
    };
    manifest.to_describe_json()
}

struct FsState {
    host: *const HostVtable,
}

// SAFETY: host 指向宿主的 'static vtable（只读），FsState 自身无可变共享状态；
// init 之后指针不再改变，跨线程共享安全。
unsafe impl Send for FsState {}
unsafe impl Sync for FsState {}

impl FsState {
    /// 经宿主 vtable 打日志（与 tool-echo 同款演示）。
    fn log(&self, level: u32, msg: &str) {
        // SAFETY: host 是 init 时宿主交付的 'static vtable 指针
        unsafe {
            if let Some(vt) = self.host.as_ref() {
                vt.log_str(level, msg);
            }
        }
    }
}

impl PluginState for FsState {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        self.log(log_level::INFO, &format!("tool-fs: {tool} called"));
        match tool {
            "fs.read" => fs_read(&args),
            "fs.list" => fs_list(&args),
            "fs.grep" => fs_grep(&args),
            other => Err(format!("unknown tool `{other}`")),
        }
    }
}

// ---------- 参数辅助 ----------

fn arg_path(args: &Value) -> Result<String, String> {
    args.get("path")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "missing string field `path`".to_string())
}

fn arg_u64(args: &Value, key: &str, default: u64, max: u64) -> Result<u64, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => {
            let n = v
                .as_u64()
                .ok_or_else(|| format!("field `{key}` must be a non-negative number"))?;
            Ok(n.min(max))
        }
    }
}

// ---------- fs.read ----------

fn fs_read(args: &Value) -> Result<Value, String> {
    let path = arg_path(args)?;
    let max = arg_u64(args, "max_bytes", READ_DEFAULT, READ_MAX)?;
    let meta = std::fs::metadata(&path).map_err(|e| format!("fs.read `{path}`: {e}"))?;
    if meta.is_dir() {
        return Err(format!("fs.read `{path}` is a directory (use fs.list)"));
    }
    let file = std::fs::File::open(&path).map_err(|e| format!("fs.read `{path}`: {e}"))?;
    let mut buf = Vec::new();
    file.take(max + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("fs.read `{path}`: {e}"))?;
    let truncated = buf.len() as u64 > max;
    buf.truncate(max as usize);
    Ok(json!({
        "path": path,
        "size": meta.len(),
        "truncated": truncated,
        "content": String::from_utf8_lossy(&buf).as_ref(),
    }))
}

// ---------- fs.list ----------

fn fs_list(args: &Value) -> Result<Value, String> {
    let path = arg_path(args)?;
    let rd = std::fs::read_dir(&path).map_err(|e| format!("fs.list `{path}`: {e}"))?;
    let mut entries: Vec<(bool, Value)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let ft = e.file_type().ok()?;
            let kind = if ft.is_dir() {
                "dir"
            } else if ft.is_file() {
                "file"
            } else {
                "other"
            };
            let size = e.metadata().ok().map(|m| json!(m.len()));
            Some((
                ft.is_dir(),
                json!({ "name": name, "kind": kind, "size": size }),
            ))
        })
        .collect();
    // 目录在前，其余按名字排（确定性输出，模型好读）
    entries.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1["name"].as_str().cmp(&b.1["name"].as_str()))
    });
    let entries: Vec<Value> = entries.into_iter().map(|(_, v)| v).collect();
    Ok(json!({ "path": path, "entries": entries }))
}

// ---------- fs.grep ----------

fn fs_grep(args: &Value) -> Result<Value, String> {
    let path = arg_path(args)?;
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `query`".to_string())?
        .to_lowercase();
    if query.is_empty() {
        return Err("fs.grep `query` must not be empty".into());
    }
    let max_results =
        arg_u64(args, "max_results", GREP_RESULTS_DEFAULT, GREP_RESULTS_MAX)? as usize;
    let max_depth = arg_u64(args, "max_depth", GREP_DEPTH_DEFAULT, GREP_DEPTH_MAX)?;

    let mut matches: Vec<Value> = Vec::new();
    let mut truncated = false;
    let mut scanned = 0u64;
    // 迭代栈代替递归：带深度预算的目录遍历
    let mut stack: Vec<(PathBuf, u64)> = vec![(PathBuf::from(&path), 0)];
    'outer: while let Some((cur, depth)) = stack.pop() {
        let Ok(meta) = std::fs::metadata(&cur) else {
            continue;
        };
        if meta.is_dir() {
            if depth >= max_depth {
                continue;
            }
            let Ok(rd) = std::fs::read_dir(&cur) else {
                continue;
            };
            let mut children: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
            children.sort();
            // 倒序入栈：pop 出来仍是名字序
            for child in children.into_iter().rev() {
                stack.push((child, depth + 1));
            }
            continue;
        }
        if meta.len() > GREP_FILE_MAX {
            continue; // 大文件跳过（二进制/日志），不计入 scanned
        }
        scanned += 1;
        let Ok(bytes) = std::fs::read(&cur) else {
            continue;
        };
        for (i, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
            if line.to_lowercase().contains(&query) {
                let mut text = line.to_string();
                if text.chars().count() > GREP_LINE_EXCERPT {
                    text = text.chars().take(GREP_LINE_EXCERPT).collect();
                }
                matches.push(json!({
                    "file": cur.display().to_string(),
                    "line": i + 1,
                    "text": text,
                }));
                if matches.len() >= max_results {
                    truncated = true;
                    break 'outer;
                }
            }
        }
    }
    Ok(json!({
        "query": query,
        "matches": matches,
        "truncated": truncated,
        "scanned": scanned,
    }))
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    Box::new(FsState { host })
}

plugin_api::declare_plugin!(describe_manifest, make_state);
