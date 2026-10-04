//! tool-edit：精确代码编辑插件（plugins.md P1，阶段八⑥第二批）。
//!
//! 三个工具，全部 `Compensable { undo_tool: "fs_restore" }`——每次编辑前
//! 读原文件生成精确 undo 数据（整文件备份进数据根），撤销统一走 tool-fs
//! 的 `fs_restore`（跨插件 undo：官方捆绑保证同装；备份布局与恢复契约
//! 见 tool-fs 文档，实现共享 `plugin_api::fsutil`）：
//! - `edit_replace`：old_string → new_string 精确替换（默认强制唯一命中）
//! - `edit_lines`：按 1-based 行号区间替换
//! - `edit_diff`：应用 unified diff（严格精确匹配，无 fuzz）
//!
//! 为什么比 fs_write 重要（plugins.md §三）：整文件重写会丢未预期改动、
//! 浪费 token；编辑类工具让模型只提交 diff。
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
}

use std::collections::BTreeMap;
use std::ffi::CString;

use plugin_api::ffi::log_level;
use plugin_api::manifest::{
    Author, LocalizedOrString, LocalizedString, PluginKind, PluginManifest, PluginOrigin,
};
use plugin_api::serde_json::{Value, json};
use plugin_api::tool::{SideEffect, ToolSpec};
use plugin_api::{HostVtable, PluginState};

fn zh_en(zh: &str, en: &str) -> LocalizedString {
    LocalizedString(BTreeMap::from([
        ("zh-CN".to_owned(), zh.to_owned()),
        ("en-US".to_owned(), en.to_owned()),
    ]))
}

const PLUGIN_ID: &str = "tool-edit";

fn plugin_version() -> &'static str {
    option_env!("LOCOWRK_PLUGIN_VERSION").unwrap_or("0.0.0-dev")
}

/// describe 符号的数据源：manifest JSON（NUL 结尾，约定见 plugin-api::ffi）。
fn describe_manifest() -> CString {
    let compensable = || SideEffect::Compensable {
        undo_tool: "fs_restore".into(),
    };
    let manifest = PluginManifest {
        id: PLUGIN_ID.into(),
        version: plugin_version().into(),
        abi_version: plugin_api::PLUGIN_ABI_VERSION,
        kind: PluginKind::Tool,
        tools: vec![
            ToolSpec {
                name: "edit_replace".into(),
                description: "Exact string replacement in a text file (the file must exist). \
                              Args: path (absolute), old_string (must match exactly once unless \
                              replace_all), new_string, replace_all (bool, default false). \
                              The original file is backed up automatically; the result carries \
                              {path, bytes, edits, backup} for fs_restore to undo."
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
                                         "description": "replace every occurrence (default false)" }
                    },
                    "required": ["path", "old_string", "new_string"]
                }),
                side_effect: compensable(),
                exposure: Default::default(),
            },
            ToolSpec {
                name: "edit_lines".into(),
                description: "Replace a 1-based inclusive line range in a text file (the file \
                              must exist). Args: path (absolute), start_line, end_line, content \
                              (new text for the range; empty content deletes it). \
                              The original file is backed up automatically; the result carries \
                              {path, bytes, replaced, backup} for fs_restore to undo."
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
                                     "description": "new text for the range; empty string deletes" }
                    },
                    "required": ["path", "start_line", "end_line", "content"]
                }),
                side_effect: compensable(),
                exposure: Default::default(),
            },
            ToolSpec {
                name: "edit_diff".into(),
                description: "Apply a unified diff to a text file (the file must exist). \
                              Args: path (absolute), patch (unified diff text with @@ hunks). \
                              Context lines must match exactly — no fuzz; on mismatch the tool \
                              errors and the file is untouched. The original file is backed up \
                              automatically; the result carries {path, bytes, hunks_applied, \
                              backup} for fs_restore to undo."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "path": { "type": "string",
                                  "description": "absolute file path (must be inside the allowed \
                                                  workspace roots)" },
                        "patch": { "type": "string", "description": "unified diff text" }
                    },
                    "required": ["path", "patch"]
                }),
                side_effect: compensable(),
                exposure: Default::default(),
            },
        ],
        // v2 展示元数据（packaging.md §2.2；仓库自带插件显式声明 official）
        display_name: Some(zh_en("代码编辑", "Code Edit")),
        description: Some(LocalizedOrString::Localized(zh_en(
            "精确代码编辑：字符串替换 / 行区间替换 / unified diff，全部带备份可撤销",
            "Precise edits: string replace / line range / unified diff, all undoable via backup",
        ))),
        icon: Some("assets/icon.svg".into()),
        author: Some(Author {
            name: "Locowrk Team".into(),
            url: None,
        }),
        origin: PluginOrigin::Official,
        homepage: None,
        license: Some("MIT".into()),
    };
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
