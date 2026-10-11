//! memory-sqlite：官方内置跨会话记忆插件（plugins.md 第三批 / §三）。
//!
//! 职责边界：本插件只做"跨会话记忆"（Agent 的持久笔记）；会话事件的
//! 持久化本身归 kernel `session-store`（DESIGN §6），不在此重复。
//!
//! 存储：`<数据根>/memory.sqlite3`（固定路径，模型不可指定——没有路径
//! 参数就没有路径逃逸面）。数据根 = `LOCOWRK_DATA_DIR` 或 HOME 下
//! `.Locowrk`（权威实现在 `plugin_api::fsutil`）。
//!
//! 副作用分级（plugins.md §三表格把本插件整体标为 Pure；这里按用户
//! "宁严勿松"原则对写操作收紧一档——Compensable 不触发审批、无 UX 代价，
//! 只是回滚时多一条机械可执行的 undo）：
//! - `memory_read` / `memory_list` / `memory_search` → Pure
//! - `memory_write` → `Compensable { undo_tool: "memory_undo" }`：输出携带
//!   被覆盖的旧条目，undo 精确还原
//! - `memory_delete` → `Compensable { undo_tool: "memory_undo" }`：输出携带
//!   被删条目全文，undo 重新插入
//! - `memory_undo` → Irreversible：恢复会把当前值覆盖回旧值（与 fs_restore
//!   同理，plugins.md §八"宁可标错成 Irreversible"判级）
//!
//! undo 调用契约（DESIGN §6.3）：内核以 `{"call_id", "args", "output"}` 调
//! undo_tool——FFI 边界不传 CallCtx，undo 只读原调用 `output` 里的
//! `previous`（write）或 `previous_entry`（delete）。兼容两种形态：标准契约
//! 或把 output 字段平铺在 args 顶层（手工排查用）。
//!
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。

mod store;
#[cfg(test)]
mod tests;

use std::ffi::CString;

use plugin_api::ffi::log_level;
use plugin_api::manifest::{PluginKind, PluginManifest};
use plugin_api::meta::official_meta;
use plugin_api::serde_json::{Value, json};
use plugin_api::tool::{Exposure, SideEffect, ToolSpec};
use plugin_api::{HostVtable, PluginState};

const PLUGIN_ID: &str = "memory-sqlite";

fn plugin_version() -> &'static str {
    option_env!("LOCOWRK_PLUGIN_VERSION").unwrap_or("0.0.0-dev")
}

/// describe 符号的数据源：manifest JSON（NUL 结尾，约定见 plugin-api::ffi）。
fn describe_manifest() -> CString {
    let mut manifest = PluginManifest {
        id: PLUGIN_ID.into(),
        version: plugin_version().into(),
        abi_version: plugin_api::PLUGIN_ABI_VERSION,
        kind: PluginKind::Tool,
        tools: vec![
            ToolSpec {
                name: "memory_write".into(),
                description: "Store a value under a key in the persistent memory database \
                              (survives across sessions). Args: key (required, unique \
                              identifier), value (required, text), tags (optional, short \
                              label), scope (optional: \"project\" default | \"global\" for \
                              user preferences that apply across ALL projects, e.g. \"always \
                              use pnpm\"). Overwrites any existing value; the overwritten \
                              entry is returned in `previous` and can be restored via \
                              memory_undo. Project entries are scoped to the current project: \
                              the same key in another project holds a different value. An \
                              unassociated session cannot write project memories (associate a \
                              project first, or pass scope=\"global\"). Use it for facts, \
                              decisions and user preferences worth remembering; do NOT store \
                              secrets or large blobs."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "key": { "type": "string", "description": "unique identifier, max 512 bytes; use dotted namespaces like `user.language`" },
                        "value": { "type": "string", "description": "text to store, max 256KiB" },
                        "tags": { "type": "string", "description": "optional short label, searchable" },
                        "scope": { "type": "string", "enum": ["project", "global"], "description": "project (default) = current project only; global = user preference visible in every project" }
                    },
                    "required": ["key", "value"]
                }),
                side_effect: SideEffect::Compensable {
                    undo_tool: "memory_undo".into(),
                },
                // §4.4 记忆工具面三件套始终可用（⑫ Top-N 截断时不受挤出）
                exposure: Exposure::Always,
            },
            ToolSpec {
                name: "memory_read".into(),
                description: "Read one memory entry by exact key. Looks in the current \
                              project first, then falls back to global user preferences. \
                              Returns {found, key, value, tags, updated_at, scope: \
                              \"project\"|\"global\"}; found=false when absent from both. Use on demand when the request \
                              touches prior context — not as an opening step."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "key": { "type": "string", "description": "exact key to read" }
                    },
                    "required": ["key"]
                }),
                side_effect: SideEffect::Pure,
                exposure: Exposure::Always,
            },
            ToolSpec {
                name: "memory_list".into(),
                description: "List memory entries of the current project plus global user \
                              preferences, ordered by key; each entry carries scope \
                              (\"project\"|\"global\"). Args: prefix (optional key-prefix \
                              filter), limit (default 50, max 200), offset (default 0). \
                              Returns {total, entries: [{key, value, tags, updated_at, \
                              scope}]} where total counts ALL matching entries."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "prefix": { "type": "string", "description": "optional key prefix filter" },
                        "limit": { "type": "number", "description": "default 50, max 200" },
                        "offset": { "type": "number", "description": "default 0" }
                    }
                }),
                side_effect: SideEffect::Pure,
                exposure: Default::default(),
            },
            ToolSpec {
                name: "memory_search".into(),
                description: "Substring search across keys, values and tags of the current \
                              project plus global user preferences (ASCII case-insensitive). \
                              Args: query (required), limit (default 20, max 100). Returns \
                              {query, matches: [{key, value, tags, updated_at, scope}]} — \
                              scope tells project records from global preferences. Wildcards \
                              in the query are matched literally. Use on demand when the request \
                              touches prior context — not as an opening step."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "substring to search for; % and _ are literal" },
                        "limit": { "type": "number", "description": "default 20, max 100" }
                    },
                    "required": ["query"]
                }),
                side_effect: SideEffect::Pure,
                exposure: Exposure::Always,
            },
            ToolSpec {
                name: "memory_delete".into(),
                description: "Delete one memory entry by exact key from the CURRENT project \
                              scope only. Global user preferences cannot be deleted here — \
                              they are managed from the human memory management page. The \
                              deleted entry is returned in `previous_entry` and can be \
                              restored via memory_undo; deleting a missing key is a no-op."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "key": { "type": "string", "description": "exact key to delete" }
                    },
                    "required": ["key"]
                }),
                side_effect: SideEffect::Compensable {
                    undo_tool: "memory_undo".into(),
                },
                exposure: Default::default(),
            },
            ToolSpec {
                name: "memory_undo".into(),
                description: "Undo a previous memory_write or memory_delete by restoring the \
                              entry captured in its result. Can be invoked directly after \
                              a mistaken write/delete. Args mirror the compensation \
                              payload {call_id, args, output}; only output.previous (for \
                              write) or output.previous_entry (for delete) is used. \
                              Restoring overwrites the entry's current content with the \
                              old value — classified Irreversible, so approval follows \
                              the session's permission mode (asked by default, \
                              auto-approved in full-access mode)."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "call_id": { "type": "string", "description": "id of the original memory_write / memory_delete call" },
                        "args": { "type": "object", "description": "original call args" },
                        "output": { "type": "object",
                                    "description": "original call result carrying previous or previous_entry" }
                    }
                }),
                side_effect: SideEffect::Irreversible,
                exposure: Default::default(),
            },
        ],
        // 展示元数据（display_name / description / icon / author /
        // origin / homepage / license）由 plugin_api::meta 单点给出（审计 F7）。
        ..Default::default()
    };
    official_meta(
        &mut manifest,
        ("持久记忆", "Persistent Memory"),
        (
            "跨会话持久记忆：写 / 读 / 列举 / 搜索 / 删除（写入可机械撤销）",
            "Cross-session memory: write / read / list / search / delete (writes undoable)",
        ),
    );
    manifest.to_describe_json()
}

struct MemoryState {
    host: *const HostVtable,
}

// SAFETY: host 指向宿主的 'static vtable（只读），MemoryState 自身无可变
// 共享状态（连接按调用开、用完即弃）；init 之后指针不再改变，跨线程共享安全。
unsafe impl Send for MemoryState {}
unsafe impl Sync for MemoryState {}

impl MemoryState {
    /// 经宿主 vtable 打日志（与 tool-fs 同款）。
    fn log(&self, level: u32, msg: &str) {
        // SAFETY: host 是 init 时宿主交付的 'static vtable 指针
        unsafe {
            if let Some(vt) = self.host.as_ref() {
                vt.log_str(level, msg);
            }
        }
    }

    /// 当前调用的键位上下文（㉘·A·fix 双层记忆）：经 vtable
    /// `session_context` 查宿主。**三态**——查询不可用（headless 宿主 /
    /// 非调用期 / 旧宿主）= [`WsCtx::Headless`]（`''` 单一宇宙，旧行为
    /// 不变）；查询可用但会话未归属（含子 Agent 冒泡）=
    /// [`WsCtx::Unattributed`]（只见全局层、缺省项目写拒绝）；否则
    /// [`WsCtx::Project`]（归属项目根）。缓冲 1KiB 足容纳上下文 JSON。
    fn ctx(&self) -> WsCtx {
        // SAFETY: host 是 init 时宿主交付的 'static vtable 指针
        let vt = match unsafe { self.host.as_ref() } {
            Some(vt) => vt,
            None => return WsCtx::Headless,
        };
        let mut buf = [0u8; 1024];
        // SAFETY: buf 是本栈上合法可写缓冲
        match unsafe { vt.session_context(&mut buf) } {
            Ok(n) => match plugin_api::ffi::CallContext::from_json(&buf[..n]) {
                Some(c) => WsCtx::from_parts(c.session_id, c.workspace),
                None => WsCtx::Headless,
            },
            Err(_) => WsCtx::Headless,
        }
    }
}

/// 调用期键位上下文（三态；store 各工具据此裁决读写键位与全局层纪律，
/// Context 文档 §4.6 双层记忆模型）。
pub(crate) enum WsCtx {
    /// headless / 查询不可用：`''` 单一宇宙（旧行为，等价"全局层"）。
    Headless,
    /// 桌面调用但会话未归属：只见全局层；缺省项目写拒绝（没有项目的
    /// "项目记忆"不许落地污染全局层）。
    Unattributed,
    /// 归属项目根：读写本项目键位；显式 `scope="global"` 升格为偏好。
    Project(String),
}

impl WsCtx {
    /// 从 FFI 上下文裁决键位（纯函数，可测）：有项目根 = Project；
    /// 无项目根但**有会话身份** = Unattributed（桌面归属缝/子 Agent 冒泡）；
    /// 无会话身份 = Headless（一次性进程等价 headless，v2.64 ToolExposure
    /// 同判据——plugin-host 测试装配与 kernel-bin 直调都落此态）。
    pub(crate) fn from_parts(session_id: Option<String>, workspace: Option<String>) -> WsCtx {
        match workspace {
            Some(ws) if !ws.is_empty() => WsCtx::Project(ws),
            _ if session_id.as_deref().map(str::is_empty) == Some(false) => WsCtx::Unattributed,
            _ => WsCtx::Headless,
        }
    }

    /// 本会话的"当前项目键位"（未归属 / headless = `''`）。
    fn scope(&self) -> &str {
        match self {
            WsCtx::Project(ws) => ws,
            _ => "",
        }
    }
}

impl PluginState for MemoryState {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        self.log(log_level::INFO, &format!("memory-sqlite: {tool} called"));
        let ctx = self.ctx();
        match tool {
            "memory_write" => store::memory_write(&ctx, &args),
            "memory_read" => store::memory_read(&ctx, &args),
            "memory_list" => store::memory_list(&ctx, &args),
            "memory_search" => store::memory_search(&ctx, &args),
            "memory_delete" => store::memory_delete(&ctx, &args),
            "memory_undo" => store::memory_undo(&ctx, &args),
            other => Err(format!("unknown tool `{other}`")),
        }
    }
}

// ---------- 参数辅助 ----------

pub(crate) fn arg_str(args: &Value, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("missing string field `{key}`"))
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    Box::new(MemoryState { host })
}

plugin_api::declare_plugin!(describe_manifest, make_state);
