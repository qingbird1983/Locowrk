//! tool-fs：官方内置文件工具插件（plugins.md P0；阶段八③第一批落地只读，
//! 阶段八⑥第二批补写入能力）。
//!
//! 副作用分级（plugins.md §二，关键设计）：
//! - `fs_read` / `fs_list` / `fs_grep` → Pure，随便调，回退免费
//! - `fs_write` → `Compensable { undo_tool: "fs_restore" }`，写前备份原内容
//! - `fs_delete` → Irreversible，走审批（v1 只删文件，不删目录）
//! - `fs_restore` → Irreversible：恢复会把文件当前内容（可能比备份新）覆盖掉，
//!   按 plugins.md §八"宁可标错成 Irreversible"判级；补偿执行器（DESIGN
//!   §6.3）对 Irreversible 的 undo 调用同样过审批门
//!
//! 备份布局：`<数据根>/backups/tool-fs/<纳秒时间戳>-<pid>/<原文件名>`。
//! 数据根 = `LOCOWRK_DATA_DIR` 或 HOME 下 `.Locowrk`（权威共享实现在
//! `plugin_api::fsutil`）。undo 调用契约（§6.3）：内核以
//! `{"call_id", "args", "output"}` 调 undo_tool——FFI 边界不传 CallCtx，
//! 工具从原调用的 `output` 里取 {path, backup, existed}。恢复 = 把备份
//! rename 回原位（备份自消费，重复恢复是 no-op，天然幂等）。
//!
//! 安全底线（用户 2026-09-29 指定）：所有路径参数 canonicalize 后校验
//! 允许根前缀——基座在 `plugin_api::fsutil`；允许根 ㉘·B 起围栏跟会话
//! （vtable `session_context` 的 fs_roots 优先，未归属回落
//! LOCOWRK_FS_ROOTS，缺省只有数据根）；相对路径按宿主进程 CWD 解析且
//! 同样过校验。
//!
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。

mod read;
mod write;

use std::ffi::CString;

use plugin_api::ffi::log_level;
use plugin_api::manifest::{PluginKind, PluginManifest};
use plugin_api::meta::official_meta;
use plugin_api::serde_json::{Value, json};
use plugin_api::tool::{Exposure, SideEffect, ToolSpec};
use plugin_api::{HostVtable, PluginState};

const PLUGIN_ID: &str = "tool-fs";

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
                name: "fs_read".into(),
                description: "Read a UTF-8 text file from the local filesystem. \
                              Pass an absolute path. Returns {path, size, truncated, revision, \
                              content}; content is capped at max_bytes (default 32KiB) but \
                              `revision` always covers the FULL file. Read before edit_replace \
                              so old_string can be copied character-for-character, and pass the \
                              `revision` as `expected_revision` on any later fs_write / edit / \
                              fs_delete of this file — write tools reject stale observations."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "absolute file path (required; must be inside the allowed workspace roots)" },
                        "max_bytes": { "type": "number",
                                       "description": "cap returned content (default 32768, max 1048576)" }
                    },
                    "required": ["path"]
                }),
                side_effect: SideEffect::Pure,
                exposure: Exposure::Always,
            },
            ToolSpec {
                name: "fs_list".into(),
                description: "List a directory. Pass an absolute path. \
                              Returns {path, entries: [{name, kind: file|dir|other, size}]}; \
                              directories come first, then names sorted."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "absolute directory path (required; must be inside the allowed workspace roots)" }
                    },
                    "required": ["path"]
                }),
                side_effect: SideEffect::Pure,
                exposure: Default::default(),
            },
            ToolSpec {
                name: "fs_grep".into(),
                description: "Case-insensitive substring search in a text file, or \
                              recursively under a directory. Pass an absolute path. \
                              Returns {query, matches: [{file, line, text}], truncated, scanned}. \
                              Use this instead of shell grep/findstr pipelines for content \
                              search — it is scoped to the workspace roots and returns \
                              structured matches."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "absolute file or directory path (required; must be inside the allowed workspace roots)" },
                        "query": { "type": "string", "description": "case-insensitive substring" },
                        "max_results": { "type": "number", "description": "default 50, max 200" },
                        "max_depth": { "type": "number", "description": "recursion depth, default 3, max 8" }
                    },
                    "required": ["path", "query"]
                }),
                side_effect: SideEffect::Pure,
                exposure: Exposure::Always,
            },
            ToolSpec {
                name: "fs_write".into(),
                description: "Write a UTF-8 text file (create or overwrite). \
                              Args: path (absolute), content, create_dirs (bool, default false), \
                              expected_revision — the `revision` from a previous fs_read of \
                              this file; pass null ONLY to create a file that must not exist \
                              yet (the call is rejected if the file already exists without a \
                              revision, so blind overwrites are impossible). The previous \
                              content, if any, is backed up automatically; the result carries \
                              {path, bytes, existed, backup, revision} — pass that `revision` \
                              as `expected_revision` on your next write to this file, or pass \
                              the original output to fs_restore to undo."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "absolute file path (required; must be inside the allowed workspace roots)" },
                        "content": { "type": "string", "description": "full new file content" },
                        "create_dirs": { "type": "boolean",
                                         "description": "create missing parent dirs (default false)" },
                        "expected_revision": { "type": ["string", "null"],
                                               "description": "revision from fs_read or the previous write's result; null = create a new file (rejected if the file exists)" }
                    },
                    "required": ["path", "content", "expected_revision"]
                }),
                side_effect: SideEffect::Compensable {
                    undo_tool: "fs_restore".into(),
                },
                exposure: Default::default(),
            },
            ToolSpec {
                name: "fs_delete".into(),
                description: "Delete a file (directories are rejected in v1). \
                              Args: path (absolute), expected_revision (the `revision` from \
                              fs_read — deleting must prove the file hasn't changed since you \
                              observed it, so an externally modified file is never deleted \
                              blind). Irreversible — no backup is kept, the call requires \
                              user approval."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "absolute file path (required; must be inside the allowed workspace roots)" },
                        "expected_revision": { "type": "string",
                                               "description": "revision from fs_read; the call is rejected if the file changed since you observed it" }
                    },
                    "required": ["path", "expected_revision"]
                }),
                side_effect: SideEffect::Irreversible,
                exposure: Default::default(),
            },
            ToolSpec {
                name: "fs_restore".into(),
                description: "Undo a previous fs_write / edit_replace / edit_lines / edit_diff \
                              by restoring its automatic backup (the backup is consumed; \
                              restoring twice is a no-op). Either the compensation layer or \
                              you can invoke it: pass the original call's {call_id, args, \
                              output} from the transcript; only output.{path, backup, existed} \
                              is used. The result carries the restored file's `revision` — \
                              pass it as expected_revision on your next edit. Irreversible — \
                              restoring overwrites the file's current content, so the call \
                              requires user approval."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "call_id": { "type": "string", "description": "id of the original fs_write call" },
                        "args": { "type": "object", "description": "original fs_write args" },
                        "output": { "type": "object",
                                    "description": "original fs_write result carrying path/backup/existed" }
                    }
                }),
                side_effect: SideEffect::Irreversible,
                exposure: Exposure::Always,
            },
        ],
        // 展示元数据（display_name / description / icon / author /
        // origin / homepage / license）由 plugin_api::meta 单点给出（审计 F7）。
        ..Default::default()
    };
    official_meta(
        &mut manifest,
        ("文件工具", "File Tools"),
        (
            "文件读写与检索：读 / 列目录 / 搜索 / 写（带备份可撤销）/ 删除",
            "File ops: read / list / grep / write (undoable via backup) / delete",
        ),
    );
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
            "fs_read" => read::fs_read(&args, self.host),
            "fs_list" => read::fs_list(&args, self.host),
            "fs_grep" => read::fs_grep(&args, self.host),
            "fs_write" => write::fs_write(&args, self.host),
            "fs_delete" => write::fs_delete(&args, self.host),
            "fs_restore" => write::fs_restore(&args, self.host),
            other => Err(format!("unknown tool `{other}`")),
        }
    }
}

// ---------- 参数辅助（read / write 两模块共用） ----------

/// 测试共用：env 是进程级全局，read / write 两模块的 env 用例必须共用
/// 一把锁（tool-edit 的 testutil 同款——各自持锁 = 互斥失效，env 串场）。
#[cfg(test)]
pub(crate) mod testutil {
    use std::sync::Mutex;
    pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());
}

pub(crate) fn arg_path(args: &Value) -> Result<String, String> {
    args.get("path")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "missing string field `path`".to_string())
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    Box::new(FsState { host })
}

plugin_api::declare_plugin!(describe_manifest, make_state);
