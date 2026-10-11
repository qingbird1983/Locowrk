//! tool-git：官方内置 Git 操作插件（plugins.md 第三批 / §三）。
//!
//! 副作用分级（plugins.md §三，逐项落地）：
//! - `git_status` / `git_diff` / `git_log` → Pure（只 spawn git 读仓库状态）
//! - `git_commit` → `Compensable { undo_tool: "git_undo_commit" }`：输出携带
//!   新提交哈希，undo = `reset --soft HEAD~1`（改动保留在暂存区）
//! - `git_push` → Irreversible：发布到远端，走审批
//! - `git_undo_commit` → Irreversible：reset 改写仓库状态；undo 前先校验
//!   HEAD 仍指向原提交，HEAD 动过即拒绝转人工（与 fs_restore 同理，按
//!   plugins.md §八"宁可标错成 Irreversible"判级）；根提交无父，显式拒绝
//!   并在报错里给出路（真实测试 2026-10-06 P0-2：隐性失败让模型反复尝试）
//!
//! 安全底线：
//! - `repo` 参数 canonicalize 后校验允许根前缀（fsutil，与 tool-fs 同底座）
//! - `add` 路径：canonicalize（允许根内 + 仓库内）后才交给 git；相对路径
//!   按仓库根解析（不是宿主 CWD）
//! - 远端 / 分支名走字符白名单（挡 `-oProxyCommand` 式 argv 注入）；
//!   文件路径一律置于 `--` 分隔符之后
//! - 子进程环境剥掉 GIT_DIR / GIT_WORK_TREE 等重定向变量（proc.rs），
//!   `git -C` 的路径校验不被环境变量绕过；argv 直 spawn，无 shell
//!
//! undo 调用契约（DESIGN §6.3）：内核以 `{"call_id", "args", "output"}` 调
//! undo_tool——FFI 边界不传 CallCtx，undo 只读原调用 `output` 里的
//! {repo, commit}。兼容把 output 平铺在 args 顶层的形态（手工排查用）。
//!
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。

mod proc;
mod read;
#[cfg(test)]
mod tests;
mod write;

use std::ffi::CString;
use std::path::{Path, PathBuf};

use plugin_api::ffi::log_level;
use plugin_api::fsutil::{effective_roots, resolve_existing};
use plugin_api::manifest::{PluginKind, PluginManifest};
use plugin_api::meta::official_meta;
use plugin_api::serde_json::{Value, json};
use plugin_api::tool::{SideEffect, ToolSpec};
use plugin_api::{HostVtable, PluginState};

const PLUGIN_ID: &str = "tool-git";

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
                name: "git_status".into(),
                description: "Show git working-tree status: current branch, ahead/behind \
                              vs upstream, and changed entries (x = index status, y = \
                              worktree status; '??' = untracked, 'A' = staged add, 'M' = \
                              modified, 'D' = deleted). Args: repo (required)."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "repo": { "type": "string", "description": "absolute path to the repository (must be inside the allowed workspace roots)" }
                    },
                    "required": ["repo"]
                }),
                side_effect: SideEffect::Pure,
                exposure: Default::default(),
            },
            ToolSpec {
                name: "git_diff".into(),
                description: "Show the unified diff of unstaged changes (default) or \
                              staged changes. Args: repo (required), staged (bool, \
                              default false), path (optional, restrict the diff to one \
                              file). Output is capped at 256KiB and flags `truncated`."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "repo": { "type": "string", "description": "absolute path to the repository (must be inside the allowed workspace roots)" },
                        "staged": { "type": "boolean", "description": "diff the index instead of the worktree (default false)" },
                        "path": { "type": "string", "description": "optional path relative to the repo root" }
                    },
                    "required": ["repo"]
                }),
                side_effect: SideEffect::Pure,
                exposure: Default::default(),
            },
            ToolSpec {
                name: "git_log".into(),
                description: "List recent commits, newest first. Args: repo (required), \
                              limit (default 20, max 100), path (optional, only commits \
                              touching that path). Returns {commits: [{hash, short, \
                              author, date, subject}]}; empty list for a repo without \
                              commits."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "repo": { "type": "string", "description": "absolute path to the repository (must be inside the allowed workspace roots)" },
                        "limit": { "type": "number", "description": "default 20, max 100" },
                        "path": { "type": "string", "description": "optional path relative to the repo root" }
                    },
                    "required": ["repo"]
                }),
                side_effect: SideEffect::Pure,
                exposure: Default::default(),
            },
            ToolSpec {
                name: "git_commit".into(),
                description: "Stage files and create a commit. Args: repo (required), \
                              message (required), add (array of paths relative to the \
                              repo root) OR add_all (bool, stage everything including \
                              deletions) — exactly one of the two. Returns {commit, \
                              short}. Undoable via git_undo_commit (soft reset keeps the \
                              changes staged) as long as HEAD is still the created \
                              commit — except the repo's ROOT (first) commit, which has \
                              no parent and cannot be undone this way."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "repo": { "type": "string", "description": "absolute path to the repository (must be inside the allowed workspace roots)" },
                        "message": { "type": "string", "description": "commit message (first line = subject)" },
                        "add": { "type": "array", "items": { "type": "string" },
                                 "description": "paths to stage, relative to the repo root (new files allowed)" },
                        "add_all": { "type": "boolean",
                                     "description": "stage all changes including deletions (default false)" }
                    },
                    "required": ["repo", "message"]
                }),
                side_effect: SideEffect::Compensable {
                    undo_tool: "git_undo_commit".into(),
                },
                exposure: Default::default(),
            },
            ToolSpec {
                name: "git_push".into(),
                description: "Push a branch to a remote. Args: repo (required), remote \
                              (default 'origin'), branch (required). Irreversible — \
                              publishing to the remote requires user approval."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "repo": { "type": "string", "description": "absolute path to the repository (must be inside the allowed workspace roots)" },
                        "remote": { "type": "string", "description": "remote name, default 'origin'" },
                        "branch": { "type": "string", "description": "branch name to push, e.g. 'main'" }
                    },
                    "required": ["repo", "branch"]
                }),
                side_effect: SideEffect::Irreversible,
                exposure: Default::default(),
            },
            ToolSpec {
                name: "git_undo_commit".into(),
                description: "Undo a previous git_commit by soft-resetting one commit \
                              (the changes stay staged). Can be called directly after a \
                              mistaken commit. Args mirror the compensation payload \
                              {call_id, args, output}; only output.{repo, commit} is \
                              used. Refuses when HEAD has moved past the recorded commit \
                              (manual intervention required), and refuses the repo's \
                              ROOT commit (no parent to reset to — the error names the \
                              manual way out). Rewrites repository state: classified \
                              Irreversible, so approval follows the session's permission \
                              mode (asked by default, auto-approved in full-access mode)."
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "call_id": { "type": "string", "description": "id of the original git_commit call" },
                        "args": { "type": "object", "description": "original git_commit args" },
                        "output": { "type": "object",
                                    "description": "original git_commit result carrying repo/commit" }
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
        ("Git 工具", "Git Tools"),
        (
            "Git 操作：状态 / 差异 / 日志 / 提交（可撤销）/ 推送",
            "Git ops: status / diff / log / commit (undoable) / push",
        ),
    );
    manifest.to_describe_json()
}

struct GitState {
    host: *const HostVtable,
}

// SAFETY: host 指向宿主的 'static vtable（只读），GitState 自身无可变共享
// 状态（每次调用独立 spawn git 子进程）；init 之后指针不再改变。
unsafe impl Send for GitState {}
unsafe impl Sync for GitState {}

impl GitState {
    /// 经宿主 vtable 打日志（与 tool-fs 同款）。
    fn log(&self, level: u32, msg: &str) {
        // SAFETY: host 是 init 时宿主交付的 'static vtable 指针
        unsafe {
            if let Some(vt) = self.host.as_ref() {
                vt.log_str(level, msg);
            }
        }
    }
}

impl PluginState for GitState {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        self.log(log_level::INFO, &format!("tool-git: {tool} called"));
        match tool {
            "git_status" => read::git_status(&args, self.host),
            "git_diff" => read::git_diff(&args, self.host),
            "git_log" => read::git_log(&args, self.host),
            "git_commit" => write::git_commit(&args, self.host),
            "git_push" => write::git_push(&args, self.host),
            "git_undo_commit" => write::git_undo_commit(&args, self.host),
            other => Err(format!("unknown tool `{other}`")),
        }
    }
}

// ---------- 参数与路径辅助 ----------

pub(crate) fn arg_path(args: &Value) -> Result<String, String> {
    args.get("repo")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "missing string field `repo`".to_string())
}

/// repo 参数：canonicalize + 允许根校验（仓库可以是仓库根，也可以是仓库
/// 内目录——`git -C` 会向上发现 .git）。
pub(crate) fn resolve_repo(args: &Value, host: *const HostVtable) -> Result<PathBuf, String> {
    let p = arg_path(args)?;
    // ㉘·B：围栏跟会话（vtable session_context 的 fs_roots 优先，env 回落）
    let roots = unsafe { effective_roots(host) }?;
    resolve_existing(Path::new(&p), &roots)
}

/// output.repo 平铺还原用（undo 契约）。
pub(crate) fn resolve_repo_str(p: &str, host: *const HostVtable) -> Result<PathBuf, String> {
    let roots = unsafe { effective_roots(host) }?;
    resolve_existing(Path::new(p), &roots)
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    Box::new(GitState { host })
}

plugin_api::declare_plugin!(describe_manifest, make_state);
