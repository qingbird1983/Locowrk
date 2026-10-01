//! tool-sandbox：受限执行沙箱插件（plugins.md P2，ROADMAP ⑯；方案
//! 定稿与能力矩阵见 plugins.md §六"tool-sandbox 方案定稿"）。
//!
//! 定位：资源与权限的爆炸半径限制档——JobObject 限额（内存 / 进程数 /
//! 可选 CPU 占比 / 树治理 / 剪贴板 UI 限制）+ 受限令牌（DISABLE_MAX_
//! PRIVILEGE + deny Administrators，无特权、不可 UAC 提升）。**不是**
//! 文件系统/网络密封舱（Windows 无容器保证，受限令牌做不到，如实声明
//! 不做虚假安全承诺）；因此策略面与 tool-shell 同源三道闸（硬锁/高危/
//! 白名单）原样保留，且**不设高危解锁开关**——fs 不隔离前提下沙箱内
//! 解锁 rm 无增益，宁严勿松，引导走 shell 的解锁路径。与 tool-shell
//! 并存不替换。
//!
//! 单工具 `sandbox_run`：一律 Irreversible 走审批；输出带 `isolation`
//! 字段如实报告生效档（受限令牌不可用时降级 job-only，不静默）。
//!
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。

mod exec;
mod policy;
mod spawn;

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

const PLUGIN_ID: &str = "tool-sandbox";

fn plugin_version() -> &'static str {
    option_env!("LOCOWRK_PLUGIN_VERSION").unwrap_or("0.0.0-dev")
}

fn sandbox_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "command": { "type": "string",
                         "description": "single simple command, e.g. \"git status\"; \
                                         no pipes/semicolons/backticks" },
            "cwd": { "type": "string",
                     "description": "optional working directory (absolute path, must exist)" },
            "timeout_secs": { "type": "number",
                              "description": "default 20 (env LOCOWRK_SANDBOX_TIMEOUT), \
                                              max 120; whole tree killed on expiry" },
            "max_bytes": { "type": "number",
                           "description": "cap stdout/stderr each (default 65536, \
                                           env LOCOWRK_SANDBOX_MAX_BYTES, max 262144)" },
            "mem_mb": { "type": "number",
                        "description": "per-process memory cap MB (default 512, \
                                        env LOCOWRK_SANDBOX_MEM_MB, max 4096); \
                                        exceeding it fails allocations" },
            "max_procs": { "type": "number",
                           "description": "active process cap (default 64, \
                                           env LOCOWRK_SANDBOX_MAX_PROCS, max 256)" },
            "cpu_rate": { "type": "number",
                          "description": "optional hard CPU cap percent 10-99 \
                                          (default off, env LOCOWRK_SANDBOX_CPU_RATE)" }
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
        tools: vec![ToolSpec {
            name: "sandbox_run".into(),
            description: "Run ONE whitelisted command inside a resource-limited sandbox: \
                          memory cap (mem_mb), active-process cap (max_procs), optional \
                          CPU hard cap (cpu_rate), clipboard isolation, whole-tree kill \
                          on timeout, and a restricted token (no privileges, cannot \
                          elevate). IMPORTANT: filesystem and network are NOT isolated \
                          — the command reads/writes files and reaches the network with \
                          the same access as the host. Same command policy as `shell`: \
                          interpreters/wrappers/network commands are permanently locked; \
                          high-risk commands (rm, sudo, ...) are refused here with no \
                          unlock switch (the sandbox adds no filesystem isolation — use \
                          `shell` with its unlock switch instead). Whitelist: ls, cat, \
                          grep, git status; more via host config. No pipes, no command \
                          substitution. Requires user approval."
                .into(),
            schema: sandbox_schema(),
            side_effect: SideEffect::Irreversible,
            exposure: Default::default(),
        }],
        // v2 展示元数据（packaging.md §2.2；仓库自带插件显式声明 official）
        display_name: Some(zh_en("受限执行沙箱", "Sandbox")),
        description: Some(LocalizedOrString::Localized(zh_en(
            "受限档命令执行：内存/进程数/CPU 限额 + 无特权受限令牌 + 剪贴板隔离，\
             一律审批；文件系统与网络不隔离（如实声明，策略面与 shell 同源）",
            "Resource-limited command execution: memory/process/CPU caps + unprivileged \
             restricted token + clipboard isolation, always approved; filesystem and \
             network are NOT isolated (stated honestly, same policy as shell)",
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

struct SandboxState {
    host: *const HostVtable,
    /// LOCOWRK_SANDBOX_ALLOW 在 init 时读一次（进程级配置，运行期不变）。
    extra_allow: Vec<String>,
    /// 超时/输出/限额的运行缺省（模型 arg 缺省时取这里，arg 可覆盖）。
    defaults: exec::RunDefaults,
}

// SAFETY: host 指向宿主的 'static vtable（只读）；其余字段 init 后不变。
unsafe impl Send for SandboxState {}
unsafe impl Sync for SandboxState {}

impl SandboxState {
    fn log(&self, level: u32, msg: &str) {
        // SAFETY: host 是 init 时宿主交付的 'static vtable 指针
        unsafe {
            if let Some(vt) = self.host.as_ref() {
                vt.log_str(level, msg);
            }
        }
    }

    /// 启动期明示（清单 §2.4）：被清单压制的显式配置与环境缺省的非法
    /// 覆盖；并把"沙箱不隔离什么"打进启动日志（不做虚假安全声明）。
    fn log_startup_facts(&self, env_warns: &[String]) {
        let suppressed = policy::conflicting_allow(&self.extra_allow);
        if !suppressed.is_empty() {
            self.log(
                log_level::WARN,
                &format!(
                    "tool-sandbox: LOCOWRK_SANDBOX_ALLOW entries suppressed by denylist \
                     (denylist always wins): {}",
                    suppressed.join(", ")
                ),
            );
        }
        self.log(
            log_level::INFO,
            "tool-sandbox: limits = memory/process/CPU caps + clipboard isolation + \
             restricted token; filesystem and network are NOT isolated (policy \
             denylist/whitelist still applies)",
        );
        for w in env_warns {
            self.log(log_level::WARN, w);
        }
    }
}

impl PluginState for SandboxState {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        match tool {
            "sandbox_run" => {
                self.log(
                    log_level::INFO,
                    "tool-sandbox: sandbox_run called (approval path)",
                );
                exec::sandbox_exec(&args, &self.extra_allow, &self.defaults)
            }
            other => Err(format!("unknown tool `{other}`")),
        }
    }
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    let (defaults, env_warns) = exec::RunDefaults::from_env();
    let state = SandboxState {
        host,
        extra_allow: policy::extra_allow_from_env(),
        defaults,
    };
    state.log_startup_facts(&env_warns);
    Box::new(state)
}

plugin_api::declare_plugin!(describe_manifest, make_state);
