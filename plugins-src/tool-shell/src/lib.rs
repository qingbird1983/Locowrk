//! tool-shell：官方内置命令执行插件（plugins.md P0，阶段八⑥第二批）。
//!
//! **整个系统里最危险的插件**（plugins.md §二原话）：任何命令一律
//! `SideEffect::Irreversible`，走审批；策略层再前置两道闸——
//! 白名单（plugins.md §二安全策略：默认 `ls` / `cat` / `grep` /
//! `git status`，`LOCOWRK_SHELL_ALLOW` 显式扩展）与硬拒绝清单
//! （`rm` / `sudo` / `curl | sh` 面等，denylist 永远赢、不进审批）。
//!
//! 无 shell 解析：引号感知切词后**首词即程序**直接 spawn（exec.rs），
//! 管道/命令替换等元字符整条拒绝——`curl | sh` 类注入在策略层就死掉。
//! 轮询超时（缺省 20s，上限 120s）超时 kill；stdout/stderr 各自限长。
//!
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。

mod exec;
mod policy;

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

const PLUGIN_ID: &str = "tool-shell";

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
        tools: vec![ToolSpec {
            name: "shell".into(),
            description: "Run ONE whitelisted command and return {command, program, exit_code, \
                          success, timed_out, truncated, stdout, stderr}. Only a small allowlist \
                          is permitted (ls, cat, grep, git status; more via host config); \
                          destructive commands (rm, sudo, curl, ...) are on a permanent denylist \
                          and refused without approval; pipes, command substitution and shell \
                          metacharacters are rejected — pass a single simple command. \
                          The tool itself is Irreversible and requires user approval."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "command": { "type": "string",
                                 "description": "single simple command, e.g. \"git status\"; \
                                                 no pipes/semicolons/backticks" },
                    "cwd": { "type": "string",
                             "description": "optional working directory (absolute path, must exist)" },
                    "timeout_secs": { "type": "number", "description": "default 20, max 120" },
                    "max_bytes": { "type": "number",
                                   "description": "cap stdout/stderr each (default 65536, max 262144)" }
                },
                "required": ["command"]
            }),
            side_effect: SideEffect::Irreversible,
            exposure: Default::default(),
        }],
        // v2 展示元数据（packaging.md §2.2；仓库自带插件显式声明 official）
        display_name: Some(zh_en("命令终端", "Shell")),
        description: Some(LocalizedOrString::Localized(zh_en(
            "白名单命令执行：默认仅 ls / cat / grep / git status，全部需审批",
            "Whitelisted command execution: ls / cat / grep / git status by default, all approved",
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

struct ShellState {
    host: *const HostVtable,
    /// LOCOWRK_SHELL_ALLOW 在 init 时读一次（进程级配置，运行期不变）。
    extra_allow: Vec<String>,
}

// SAFETY: host 指向宿主的 'static vtable（只读）；extra_allow init 后不变。
unsafe impl Send for ShellState {}
unsafe impl Sync for ShellState {}

impl ShellState {
    fn log(&self, level: u32, msg: &str) {
        // SAFETY: host 是 init 时宿主交付的 'static vtable 指针
        unsafe {
            if let Some(vt) = self.host.as_ref() {
                vt.log_str(level, msg);
            }
        }
    }
}

impl PluginState for ShellState {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        if tool != "shell" {
            return Err(format!("unknown tool `{tool}`"));
        }
        self.log(log_level::INFO, "tool-shell: shell called");
        exec::shell_exec(&args, &self.extra_allow)
    }
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    Box::new(ShellState {
        host,
        extra_allow: policy::extra_allow_from_env(),
    })
}

plugin_api::declare_plugin!(describe_manifest, make_state);
