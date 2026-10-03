//! tool-echo：阶段三的验收插件（DESIGN.md §5.4 / §14）。
//!
//! 一个工具 `echo`（SideEffect::Pure）：
//! - `{"text": "...", "mode": "plain"}` —— 原样回显（全版本）
//! - `mode = "upper"` —— 转大写（全版本）
//! - `mode = "reverse"` —— 反转（**0.2.0 起新增**；旧版本报错，
//!   供"加载 0.1.0 → 激活 0.2.0 → 回退 0.1.0"验收观察行为差异）
//! - `mode = "panic"` —— 故意 panic，验证插件侧 catch_unwind（纪律 3）
//!
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。

use std::collections::BTreeMap;
use std::ffi::CString;

use plugin_api::ffi::log_level;
use plugin_api::manifest::{
    Author, LocalizedOrString, LocalizedString, PluginKind, PluginManifest, PluginOrigin,
};
use plugin_api::semver::Version;
use plugin_api::serde_json::{Value, json};
use plugin_api::tool::{SideEffect, ToolSpec};
use plugin_api::{HostVtable, PluginState};

/// 双语展示元数据样板（packaging.md §2.2）：仓库名 → 展示名，缺省回退
/// 链见 plugin-api::manifest::LocalizedString::pick。
fn zh_en(zh: &str, en: &str) -> LocalizedString {
    LocalizedString(BTreeMap::from([
        ("zh-CN".to_owned(), zh.to_owned()),
        ("en-US".to_owned(), en.to_owned()),
    ]))
}

const PLUGIN_ID: &str = "tool-echo";
/// reverse 模式自该版本起提供（0.2.0 = 版本基准重置后的第二档，demo 升级叙事用）。
const REVERSE_SINCE: Version = Version::new(0, 2, 0);

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
            name: "echo".into(),
            description: "echoes text back, optionally transformed".into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "text": { "type": "string" },
                    "mode": { "type": "string",
                              "enum": ["plain", "upper", "reverse", "panic"] }
                },
                "required": ["text"]
            }),
            side_effect: SideEffect::Pure,
            exposure: Default::default(),
        }],
        // ---- v2 展示元数据（packaging.md §2.2；仓库自带插件显式声明
        // official——缺省一律 community，官方标识必须显式给出）----
        display_name: Some(zh_en("回显", "Echo")),
        description: Some(LocalizedOrString::Localized(zh_en(
            "把文本原样或变换后回显",
            "Echoes text back, optionally transformed",
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

struct EchoState {
    host: *const HostVtable,
}

// SAFETY: host 指向宿主的 'static vtable（只读），EchoState 自身无可变共享状态；
// init 之后指针不再改变，跨线程共享安全。
unsafe impl Send for EchoState {}
unsafe impl Sync for EchoState {}

impl EchoState {
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

impl PluginState for EchoState {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        if tool != "echo" {
            return Err(format!("unknown tool `{tool}`"));
        }
        self.log(log_level::INFO, "tool-echo: echo called");
        let text = args
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing string field `text`".to_string())?;
        let mode = args.get("mode").and_then(Value::as_str).unwrap_or("plain");
        let version = plugin_version();
        let echoed = match mode {
            "plain" => text.to_string(),
            "upper" => text.to_uppercase(),
            "reverse" if reverse_supported(version) => text.chars().rev().collect(),
            "panic" => panic!("tool-echo: demo panic requested via mode=panic"),
            other => {
                return Err(format!(
                    "tool-echo {version} does not support mode=`{other}`"
                ));
            }
        };
        Ok(json!({ "echo": echoed, "plugin_version": version }))
    }
}

fn reverse_supported(version: &str) -> bool {
    Version::parse(version).is_ok_and(|v| v >= REVERSE_SINCE)
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    Box::new(EchoState { host })
}

plugin_api::declare_plugin!(describe_manifest, make_state);
