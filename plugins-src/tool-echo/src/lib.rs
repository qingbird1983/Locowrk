//! tool-echo：阶段三的验收插件（DESIGN.md §5.4 / §14）。
//!
//! 一个工具 `echo`（SideEffect::Pure）：
//! - `{"text": "...", "mode": "plain"}` —— 原样回显（全版本）
//! - `mode = "upper"` —— 转大写（全版本）
//! - `mode = "reverse"` —— 反转（**0.2.0 起新增**；旧版本报错，
//!   供"加载 0.1.0 → 激活 0.2.0 → 回退 0.1.0"验收观察行为差异）
//! - `mode = "panic"` —— 故意 panic，验证插件侧 catch_unwind（纪律 3）
//!
//! 本插件是**机制夹具**：两个版本目录由同一份源码构建（`build.rs` 只注入版本串），
//! 行为差异全部来自下面 `reverse_supported()` 的**运行时**版本比较。所以"回退后
//! reverse 不可用"证明的是 `active` 指针切换 + 插件自维的版本闸门，不是磁盘上存着
//! 旧二进制——开发态暂存面也因此只给业务插件 stage 当前版本（v2.162 F15-A，见
//! `scripts/build_plugins.sh` 头注；市场侧夹具 tool-shout 的 `repeat_supported()` 同理）。
//!
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。

#[cfg(test)]
mod tests;

use std::ffi::CString;

use plugin_api::ffi::log_level;
use plugin_api::manifest::{PluginKind, PluginManifest};
use plugin_api::meta::official_meta;
use plugin_api::plugin_version;
use plugin_api::semver::Version;
use plugin_api::serde_json::{Value, json};
use plugin_api::tool::{SideEffect, ToolSpec};
use plugin_api::{HostVtable, PluginState};

const PLUGIN_ID: &str = "tool-echo";
/// reverse 模式自该版本起提供（0.2.0 = 版本基准重置后的第二档，demo 升级叙事用）。
const REVERSE_SINCE: Version = Version::new(0, 2, 0);

/// describe 符号的数据源：manifest JSON（NUL 结尾，约定见 plugin-api::ffi）。
fn describe_manifest() -> CString {
    let mut manifest = PluginManifest {
        id: PLUGIN_ID.into(),
        version: plugin_version!().into(),
        abi_version: plugin_api::PLUGIN_ABI_VERSION,
        kind: PluginKind::Tool,
        tools: vec![ToolSpec {
            name: "echo".into(),
            // 描述取清单里那份完整句（它列全了 schema enum 的四个模式）：台账 T11 之后
            // 两份必须同源，所以补到代码侧，而不是把清单削短。
            description:
                "echoes text back, optionally transformed (modes: plain/upper/reverse/panic)".into(),
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
        // 展示元数据（display_name / description / icon / author /
        // origin / homepage / license）由 plugin_api::meta 单点给出（审计 F7）。
        ..Default::default()
    };
    official_meta(
        &mut manifest,
        ("回显", "Echo"),
        (
            "把文本原样或变换后回显",
            "Echoes text back, optionally transformed",
        ),
    );
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
        let version = plugin_version!();
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
