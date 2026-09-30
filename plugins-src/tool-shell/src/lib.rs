//! tool-shell：官方内置命令执行插件（plugins.md P0，阶段八⑥第二批；
//! S1 按《Setting and Constrained Optimization.md》§2/§3 分级治理）。
//!
//! 双工具形态：
//! - **`shell`**：审批工具，一律 `SideEffect::Irreversible` 走审批。
//!   策略三道闸——硬锁清单（解释器/包装/网络/执行面，永不放行）、
//!   高危清单（rm/sudo/chmod/dd 等，默认拒绝，解锁开关开启后放行——
//!   工具本身 Irreversible，放行即进审批）、白名单（默认 `ls` / `cat`
//!   / `grep` / `git status`，`LOCOWRK_SHELL_ALLOW` 显式扩展）。
//! - **`shell_read`**：只读工具，`SideEffect::Pure` 免审批。只放行
//!   `ls` / `cat` / `grep` / `git status|log|diff|show`，路径面经
//!   paths.rs 按 fsutil 允许根校验（与 fs_* 工具同源，cwd 与参数出根
//!   即拒并引导改用 `shell`）——否则 shell 的 cat 绕过 fs 守卫读任意
//!   盘文件，只读豁免不成立。
//!
//! 无 shell 解析：引号感知切词后**首词即程序**直接 spawn（exec.rs），
//! 管道/命令替换等元字符整条拒绝；超时杀整棵进程树（Job Object /
//! 进程组）；输出截断附提示。启动期把 `LOCOWRK_SHELL_ALLOW` 被清单
//! 压制的条目与解锁开关状态打进 warn（清单 §2.4，不静默）。
//!
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。

mod exec;
mod paths;
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

fn shell_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "command": { "type": "string",
                         "description": "single simple command, e.g. \"git status\"; \
                                         no pipes/semicolons/backticks" },
            "cwd": { "type": "string",
                     "description": "optional working directory (absolute path, must exist)" },
            "timeout_secs": { "type": "number",
                              "description": "default 20 (env LOCOWRK_SHELL_TIMEOUT), max 120" },
            "max_bytes": { "type": "number",
                           "description": "cap stdout/stderr each (default 65536, \
                                           env LOCOWRK_SHELL_MAX_BYTES, max 262144)" }
        },
        "required": ["command"]
    })
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
                name: "shell".into(),
                description: "Run ONE whitelisted command and return {command, program, \
                              exit_code, success, timed_out, truncated, stdout, stderr}. \
                              Only a small allowlist is permitted (ls, cat, grep, git status; \
                              more via host config). Denylist: interpreter/wrapper/network \
                              commands (python, bash, curl, xargs, ...) are never allowed; \
                              high-risk user-judgeable commands (rm, sudo, chmod, ...) are \
                              refused unless the host unlock switch is on — they then still \
                              require user approval. Pipes, command substitution and shell \
                              metacharacters are rejected — pass a single simple command. \
                              For read-only work prefer `shell_read` (no approval needed)."
                    .into(),
                schema: shell_schema(),
                side_effect: SideEffect::Irreversible,
                exposure: Default::default(),
            },
            ToolSpec {
                name: "shell_read".into(),
                description: "Run ONE read-only command WITHOUT user approval: ls, cat, \
                              grep, or git status/log/diff/show. Same result shape as \
                              `shell`. Paths must stay inside the allowed file roots \
                              (LOCOWRK_FS_ROOTS, default = app data root) — this includes \
                              the optional cwd argument; anything resolving outside is \
                              refused: re-run the same command via the `shell` tool, which \
                              asks the user for approval. No pipes, no command \
                              substitution, no shell metacharacters."
                    .into(),
                schema: shell_schema(),
                side_effect: SideEffect::Pure,
                exposure: Default::default(),
            },
        ],
        // v2 展示元数据（packaging.md §2.2；仓库自带插件显式声明 official）
        display_name: Some(zh_en("命令终端", "Shell")),
        description: Some(LocalizedOrString::Localized(zh_en(
            "分级命令执行：只读命令免审批（shell_read），其余白名单命令需审批（shell）；硬锁解释器/网络类",
            "Tiered command execution: read-only commands run without approval (shell_read), \
             whitelisted ones need approval (shell); interpreters/network are hard-locked",
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
    /// 高危解锁开关（init 读一次；桌面端由设置 shell.unlockRisky 在插件
    /// 加载前注入，改动后重启生效）。
    unlock_risky: bool,
    /// 超时/输出限长的运行缺省（S2；模型 arg 缺省时取这里，arg 可覆盖）。
    defaults: exec::RunDefaults,
}

// SAFETY: host 指向宿主的 'static vtable（只读）；其余字段 init 后不变。
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

    /// 启动期明示（清单 §2.4）：被清单压制的显式配置、解锁开关状态与
    /// 环境默认的非法覆盖。
    fn log_startup_facts(&self, env_warns: &[String]) {
        let suppressed = policy::conflicting_allow(&self.extra_allow);
        if !suppressed.is_empty() {
            self.log(
                log_level::WARN,
                &format!(
                    "tool-shell: LOCOWRK_SHELL_ALLOW entries suppressed by denylist \
                     (denylist always wins): {}",
                    suppressed.join(", ")
                ),
            );
        }
        if self.unlock_risky {
            self.log(
                log_level::WARN,
                "tool-shell: high-risk unlock is ENABLED — rm/sudo-class commands will \
                 go to user approval instead of being refused",
            );
        }
        for w in env_warns {
            self.log(log_level::WARN, w);
        }
    }
}

impl PluginState for ShellState {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        match tool {
            "shell" => {
                self.log(log_level::INFO, "tool-shell: shell called (approval path)");
                exec::shell_exec(&args, &self.extra_allow, self.unlock_risky, &self.defaults)
            }
            "shell_read" => {
                self.log(log_level::INFO, "tool-shell: shell_read called (readonly)");
                exec::shell_read_exec(&args, &self.defaults)
            }
            other => Err(format!("unknown tool `{other}`")),
        }
    }
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    let (defaults, env_warns) = exec::RunDefaults::from_env();
    let state = ShellState {
        host,
        extra_allow: policy::extra_allow_from_env(),
        unlock_risky: policy::unlock_risky_from_env(),
        defaults,
    };
    state.log_startup_facts(&env_warns);
    Box::new(state)
}

plugin_api::declare_plugin!(describe_manifest, make_state);
