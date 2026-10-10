//! tool-shell：官方内置命令执行插件（plugins.md P0，阶段八⑥第二批；
//! v2.145 起按《Permission Model Redesign (ZCode Alignment) v1》对齐
//! ZCode 权限模型——无命令级禁令）。
//!
//! 双工具形态：
//! - **`shell`**：审批工具，一律 `SideEffect::Irreversible` 走审批。
//!   策略仅剩**结构性拒绝**（元字符 / 空命令 / 引号未闭合），其余一切
//!   命令放行进审批卡；解释器/高危/网络类的安全语义由审批卡 + 粘性
//!   规则（桌面 permission_rules）承载，审批管「怎么用」不做「能不能
//!   用」级禁令。
//! - **`shell_read`**：只读工具，`SideEffect::Pure` 免审批。放行前提
//!   是**可证明只读**（proof.rs，G4 只读证明层，可靠性稿 §6）：词法
//!   （引号感知，未加引号/双引号内的元字符是结构面）→ 语法（单条
//!   简单命令）→ 语义（固定表 `ls` / `cat` / `grep` /
//!   `git status|log|diff|show|--version` + 精确 argv 表 `pwd` /
//!   `node -v|--version` / `python[3] --version` + git 全局选项拒
//!   （`-c` 可借配置触发外部程序）+ 写/外部程序旗标拒
//!   （`--output`/`--ext-diff`）+ 敏感凭据路径拒（`.env`/`.ssh`/
//!   credentials）。三态 Safe/Unsafe/Unknown，
//!   Unknown 按 Unsafe 处理——证明不了就拒绝并指路 `shell` 审批。
//!   路径面经 paths.rs 按 fsutil 允许根校验（与 fs_* 工具同源，cwd 与
//!   参数出根即拒）——否则 shell 的 cat 绕过 fs 守卫读任意盘文件，
//!   只读豁免不成立。
//!
//! 无 shell 解析：**首词即程序**直接 spawn（exec.rs），管道/命令替换
//! 等复合形态在两个工具面都不可执行；超时杀整棵进程树（Job Object /
//! 进程组）；输出截断附提示。退役 env（LOCOWRK_SHELL_ALLOW /
//! LOCOWRK_SHELL_UNLOCK_RISK，v2.145 幻影化）设置即启动 warn 一次。
//!
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。

mod exec;
mod paths;
mod policy;
mod proof;

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
                description: "Run ONE simple command and return {command, program, \
                              exit_code, success, timed_out, truncated, stdout, stderr}. \
                              Any single simple command is allowed — it will ask the user \
                              for approval first (approve once, for this session, or \
                              always for this project). No command is banned outright, \
                              but prefer `shell_read` for read-only work (no approval \
                              needed). Pipes, command substitution and shell \
                              metacharacters are rejected — pass a single simple command. \
                              Commands run WITHOUT a shell: the first word is spawned \
                              directly via PATH. On Windows hosts PATH usually resolves \
                              to Unix coreutils (Git's usr/bin) — use Unix-style commands; \
                              cmd builtins (dir, del, copy, `rmdir /s /q`) do not exist."
                    .into(),
                schema: shell_schema(),
                side_effect: SideEffect::Irreversible,
                exposure: Default::default(),
            },
            ToolSpec {
                name: "shell_read".into(),
                description: "Run ONE PROVEN read-only command WITHOUT user approval: ls, \
                              cat, grep, git status/log/diff/show/--version, pwd, \
                              node -v/--version, python/python3 --version. A proof layer \
                              parses the command (lexing + structure + semantics): it must \
                              be a single simple command — no unquoted pipes/redirection/\
                              `$()`/backticks/subshells, no `$` or backticks inside double \
                              quotes; no git global options (`-c`, `--git-dir`, ... can \
                              launch external programs via config); no write or \
                              external-program flags (`--output`, `--ext-diff`, \
                              `--textconv`); no credential paths (.env, .ssh, \
                              credentials, key material). Anything that cannot be PROVEN \
                              read-only is refused: re-run via the `shell` tool (asks the \
                              user) or pass a simpler command. Same result shape as \
                              `shell`. Paths must stay inside the allowed file roots \
                              (LOCOWRK_FS_ROOTS, default = app data root) — this includes \
                              the optional cwd argument; anything resolving outside is \
                              refused. Commands run WITHOUT a shell: the first word is \
                              spawned directly via PATH. On Windows hosts PATH usually \
                              resolves to Unix coreutils (Git's usr/bin) — use Unix-style \
                              commands; cmd builtins (dir, del, copy, `rmdir /s /q`) do \
                              not exist."
                    .into(),
                schema: shell_schema(),
                side_effect: SideEffect::Pure,
                exposure: Default::default(),
            },
        ],
        // v2 展示元数据（packaging.md §2.2；仓库自带插件显式声明 official）
        display_name: Some(zh_en("命令终端", "Shell")),
        description: Some(LocalizedOrString::Localized(zh_en(
            "分级命令执行：可证明只读的命令免审批（shell_read，解析器证明层），其余命令进审批卡；无命令级禁令",
            "Tiered command execution: provably read-only commands run without approval \
             (shell_read, parser proof layer); everything else goes to user approval \
             (shell); no command is banned outright",
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

    /// 启动期明示（清单 §2.4，不静默）：退役 env 幻影化提示 + 环境默认
    /// 的非法覆盖。
    fn log_startup_facts(&self, env_warns: &[String]) {
        for w in policy::retired_env_warns().iter().chain(env_warns.iter()) {
            self.log(log_level::WARN, w);
        }
    }
}

impl PluginState for ShellState {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        match tool {
            "shell" => {
                self.log(log_level::INFO, "tool-shell: shell called (approval path)");
                exec::shell_exec(&args, &self.defaults)
            }
            "shell_read" => {
                self.log(log_level::INFO, "tool-shell: shell_read called (readonly)");
                exec::shell_read_exec(&args, &self.defaults, self.host)
            }
            other => Err(format!("unknown tool `{other}`")),
        }
    }
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    let (defaults, env_warns) = exec::RunDefaults::from_env();
    let state = ShellState { host, defaults };
    state.log_startup_facts(&env_warns);
    Box::new(state)
}

plugin_api::declare_plugin!(describe_manifest, make_state);
