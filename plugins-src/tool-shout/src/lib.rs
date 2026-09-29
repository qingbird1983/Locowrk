//! tool-shout：阶段七的验收插件（DESIGN.md §11 / §13.3）。
//!
//! 一个工具 `shout`（SideEffect::Pure）：
//! - `{"text": "..."}` 或 `mode = "plain"` —— 转大写并加 "!"（全版本）
//! - `mode = "repeat"` —— 先重复两遍再喊（**1.1.0 起新增**；旧版本报错，
//!   供"安装 1.0.0 → 更新 1.1.0 → 回退 1.0.0"的 UI 验收观察行为差异）
//!
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。
//! 与 tool-echo 的差别：它是"从市场安装"的演示品，不进开发态 plugins/。

use std::ffi::CString;

use plugin_api::ffi::log_level;
use plugin_api::manifest::{PluginKind, PluginManifest};
use plugin_api::semver::Version;
use plugin_api::serde_json::{Value, json};
use plugin_api::tool::{SideEffect, ToolSpec};
use plugin_api::{HostVtable, PluginState};

const PLUGIN_ID: &str = "tool-shout";
/// repeat 模式自该版本起提供。
const REPEAT_SINCE: Version = Version::new(1, 1, 0);

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
            name: "shout".into(),
            description: "shouts text back in uppercase (optionally repeated)".into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "mode": { "type": "string", "enum": ["plain", "repeat"] }
                },
                "required": ["text"]
            }),
            side_effect: SideEffect::Pure,
        }],
    };
    manifest.to_describe_json()
}

struct ShoutState {
    host: *const HostVtable,
}

// SAFETY: host 指向宿主的 'static vtable（只读），ShoutState 自身无可变
// 共享状态；init 之后指针不再改变，跨线程共享安全。
unsafe impl Send for ShoutState {}
unsafe impl Sync for ShoutState {}

impl ShoutState {
    /// 经宿主 vtable 打日志（演示 HostVtable 的实际使用，§5.1）。
    fn log(&self, level: u32, msg: &str) {
        // SAFETY: host 是 init 时宿主交付的 'static vtable 指针
        unsafe {
            if let Some(vt) = self.host.as_ref() {
                vt.log_str(level, msg);
            }
        }
    }
}

impl PluginState for ShoutState {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        if tool != "shout" {
            return Err(format!("unknown tool `{tool}`"));
        }
        self.log(log_level::INFO, "tool-shout: shout called");
        let text = args
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing string field `text`".to_string())?;
        let mode = args.get("mode").and_then(Value::as_str).unwrap_or("plain");
        let version = plugin_version();
        let shouted = match mode {
            "plain" => format!("{text}!"),
            "repeat" if repeat_supported(version) => format!("{text}! {text}!"),
            other => {
                return Err(format!(
                    "tool-shout {version} does not support mode=`{other}`"
                ));
            }
        };
        Ok(json!({
            "shout": shouted.to_uppercase(),
            "plugin_version": version
        }))
    }
}

fn repeat_supported(version: &str) -> bool {
    Version::parse(version).is_ok_and(|v| v >= REPEAT_SINCE)
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    Box::new(ShoutState { host })
}

plugin_api::declare_plugin!(describe_manifest, make_state);
