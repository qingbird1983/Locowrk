//! tool-edit：精确代码编辑插件（plugins.md P1，阶段八⑥第二批）。
//!
//! 三个工具，全部 `Compensable { undo_tool: "fs_restore" }`——每次编辑前
//! 读原文件生成精确 undo 数据（整文件备份进数据根），撤销统一走 tool-fs
//! 的 `fs_restore`（跨插件 undo：官方捆绑保证同装；备份布局与恢复契约
//! 见 tool-fs 文档，实现共享 `plugin_api::fsutil`）：
//! - `edit_replace`：old_string → new_string 替换——**编辑首选**，exposure
//!   always（v2.114，设计文档 docs/design/edit_replace.md：精确零命中后
//!   容错匹配行尾空白/CRLF/BOM；Timo 实测教训 = on_demand 截断下模型
//!   根本用不上它，退而整文件重写）。唯一性仍是安全底线。
//! - `edit_lines`：按 1-based 行号区间替换
//! - `edit_diff`：应用 unified diff（严格精确匹配，无 fuzz）——结构性
//!   大改次选，description 与 edit_replace 互指优先级
//!
//! 为什么比 fs_write 重要（plugins.md §三）：整文件重写会丢未预期改动、
//! 浪费 token；编辑类工具让模型只提交 diff。
//!
//! Revision Guard（可靠性稿 §3，G3 批）：三个工具都必带 `expected_revision`
//! ——fs_read（或上一次编辑输出）里的 `revision` 与当前内容不符即拒
//! （StaleObservation 报文带双方 revision 并指路重读）；成功输出带新
//! `revision` 供链式下一次编辑，守卫不断链。
//!
//! 安全底线：路径过 `plugin_api::fsutil` 允许根校验（同 tool-fs，调用期
//! 围栏跟会话——vtable session_context 优先，env `LOCOWRK_FS_ROOTS` 回落）；
//! 目标文件必须已存在（新建走 fs_write）。
//!
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。

mod diff;
mod lines;
mod replace;

#[cfg(test)]
mod tests;

/// 测试共用：env 是进程级全局，跨模块的 env 用例必须共用一把锁。
#[cfg(test)]
pub(crate) mod testutil {
    use std::sync::Mutex;
    pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());

    /// 直呼函数的单测没有调用期 vtable——空指针 = `context_roots` 返回
    /// None，回落 env 全局围栏（与未接围栏前的语义一致）。
    pub(crate) fn no_host() -> *const plugin_api::HostVtable {
        std::ptr::null()
    }

    /// 测试里的「当前 revision」：外部 fs::write 之后取一次随调用传入
    /// （模拟模型从 fs_read / 上一次编辑输出拿到的值）。
    pub(crate) fn rev_of(path: &std::path::Path) -> String {
        plugin_api::fsutil::file_revision(path).unwrap().unwrap()
    }

    /// 每簇一个临时数据根（tag 防撞名）：`LOCOWRK_DATA_DIR` + 允许根围栏都按它建。
    pub(crate) fn temp_root(tag: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "locowrk-tool-edit-{tag}-{}-{nanos}",
            std::process::id()
        ))
    }
}

use std::ffi::CString;

use plugin_api::ffi::log_level;
use plugin_api::manifest::{PluginKind, PluginManifest};
use plugin_api::meta::official_meta;
use plugin_api::plugin_version;
use plugin_api::serde_json::{Value, json};
use plugin_api::tool::{Exposure, SideEffect, ToolSpec};
use plugin_api::{HostVtable, PluginState};

const PLUGIN_ID: &str = "tool-edit";

/// describe 符号的数据源：manifest JSON（NUL 结尾，约定见 plugin-api::ffi）。
fn describe_manifest() -> CString {
    let compensable = || SideEffect::Compensable {
        undo_tool: "fs_restore".into(),
    };
    let mut manifest = PluginManifest {
        id: PLUGIN_ID.into(),
        version: plugin_version!().into(),
        abi_version: plugin_api::PLUGIN_ABI_VERSION,
        kind: PluginKind::Tool,
        tools: vec![
            ToolSpec {
                name: "edit_replace".into(),
                description: "Replace a string in a text file (the file must exist). PREFERRED \
                              editing tool for most edits: copy old_string character-for-\
                              character from fs_read output; fall back to edit_diff only for \
                              structural changes across many places, edit_lines only with known \
                              line numbers. Args: path (absolute), old_string (must match exactly \
                              once unless replace_all), new_string, replace_all (bool, default \
                              false), expected_revision (the `revision` from fs_read or from \
                              the previous edit's result — the call is rejected with a stale-\
                              observation error if the file changed since you observed it; \
                              re-read and rebuild). If the exact match fails, a tolerant pass \
                              retries ignoring line-ending (CRLF/LF), trailing-whitespace and \
                              BOM differences — the result reports match_type so you can tell; \
                              uniqueness is always enforced (multi-match errors list candidate \
                              lines). The original file is backed up automatically; the result \
                              carries {path, bytes, edits, match_type, byte_range, backup, \
                              revision} — pass that `revision` to your next edit of this file, \
                              and the output to fs_restore to undo."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string",
                                  "description": "absolute file path (must be inside the allowed \
                                                  workspace roots)" },
                        "old_string": { "type": "string", "description": "exact text to find" },
                        "new_string": { "type": "string", "description": "replacement text" },
                        "replace_all": { "type": "boolean",
                                         "description": "replace every occurrence (default false)" },
                        "expected_revision": { "type": "string",
                                               "description": "revision from fs_read or the previous edit's result; rejected if stale" }
                    },
                    "required": ["path", "old_string", "new_string", "expected_revision"]
                }),
                side_effect: compensable(),
                exposure: Exposure::Always,
            },
            ToolSpec {
                name: "edit_lines".into(),
                description: "Replace a 1-based inclusive line range in a text file (the file \
                              must exist). Args: path (absolute), start_line, end_line, content \
                              (new text for the range; empty content deletes it), \
                              expected_revision (the `revision` from fs_read or from the \
                              previous edit's result; stale observations are rejected — \
                              re-read and rebuild). \
                              The original file is backed up automatically; the result carries \
                              {path, bytes, replaced, backup, revision} for fs_restore to undo \
                              and for your next edit of this file."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string",
                                  "description": "absolute file path (must be inside the allowed \
                                                  workspace roots)" },
                        "start_line": { "type": "number", "description": "1-based, inclusive" },
                        "end_line": { "type": "number", "description": "1-based, inclusive" },
                        "content": { "type": "string",
                                     "description": "new text for the range; empty string deletes" },
                        "expected_revision": { "type": "string",
                                               "description": "revision from fs_read or the previous edit's result; rejected if stale" }
                    },
                    "required": ["path", "start_line", "end_line", "content", "expected_revision"]
                }),
                side_effect: compensable(),
                exposure: Default::default(),
            },
            ToolSpec {
                name: "edit_diff".into(),
                description: "Apply a unified diff to a text file (the file must exist). \
                              Prefer edit_replace for most edits; use this only for structural \
                              changes across many places. Args: path (absolute), patch (unified \
                              diff text with @@ hunks), expected_revision (the `revision` from \
                              fs_read or from the previous edit's result; stale observations are \
                              rejected — re-read and rebuild). Context lines must match exactly \
                              — no fuzz; on mismatch the tool errors and the file is untouched. \
                              The original file is backed up automatically; the result carries \
                              {path, bytes, hunks_applied, backup, revision} for fs_restore to \
                              undo and for your next edit of this file."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string",
                                  "description": "absolute file path (must be inside the allowed \
                                                  workspace roots)" },
                        "patch": { "type": "string", "description": "unified diff text" },
                        "expected_revision": { "type": "string",
                                               "description": "revision from fs_read or the previous edit's result; rejected if stale" }
                    },
                    "required": ["path", "patch", "expected_revision"]
                }),
                side_effect: compensable(),
                exposure: Default::default(),
            },
        ],
        // 展示元数据（display_name / description / icon / author /
        // origin / homepage / license）由 plugin_api::meta 单点给出（审计 F7）。
        ..Default::default()
    };
    official_meta(
        &mut manifest,
        ("代码编辑", "Code Edit"),
        (
            "精确代码编辑：字符串替换 / 行区间替换 / unified diff，全部带备份可撤销",
            "Precise edits: string replace / line range / unified diff, all undoable via backup",
        ),
    );
    manifest.to_describe_json()
}

struct EditState {
    host: *const HostVtable,
}

// SAFETY: host 指向宿主的 'static vtable（只读），EditState 自身无可变共享
// 状态；init 之后指针不再改变，跨线程共享安全。
unsafe impl Send for EditState {}
unsafe impl Sync for EditState {}

impl EditState {
    fn log(&self, level: u32, msg: &str) {
        // SAFETY: host 是 init 时宿主交付的 'static vtable 指针
        unsafe {
            if let Some(vt) = self.host.as_ref() {
                vt.log_str(level, msg);
            }
        }
    }
}

impl PluginState for EditState {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        self.log(log_level::INFO, &format!("tool-edit: {tool} called"));
        match tool {
            "edit_replace" => replace::edit_replace(&args, self.host),
            "edit_lines" => lines::edit_lines(&args, self.host),
            "edit_diff" => diff::edit_diff(&args, self.host),
            other => Err(format!("unknown tool `{other}`")),
        }
    }
}

// ---------- 共用助手 ----------

/// 读 UTF-8 文本 + 行视图（lines.rs / diff.rs 共用）。返回 (原文, 行,
/// 是否以换行结尾)；行视图剥掉了结尾的空元素，写回时按 had_nl 还原。
pub(crate) fn load_lines(path: &str) -> Result<(String, Vec<String>, bool), String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("read `{path}`: {e}"))?;
    let had_nl = content.ends_with('\n');
    let mut lines: Vec<String> = content.split('\n').map(str::to_owned).collect();
    if had_nl {
        lines.pop();
    }
    Ok((content, lines, had_nl))
}

/// 写回行视图（按 had_nl 还原结尾换行）。
pub(crate) fn store_lines(path: &str, lines: &[String], had_nl: bool) -> Result<usize, String> {
    let mut out = lines.join("\n");
    if had_nl && !out.ends_with('\n') {
        out.push('\n');
    }
    std::fs::write(path, &out).map_err(|e| format!("write `{path}`: {e}"))?;
    Ok(out.len())
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    Box::new(EditState { host })
}

plugin_api::declare_plugin!(describe_manifest, make_state);
