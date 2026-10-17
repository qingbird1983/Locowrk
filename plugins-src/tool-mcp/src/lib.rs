//! tool-mcp：官方内置 MCP 桥接插件（plugins.md 第三批 / §五"生态兼容的
//! 作弊路径"）。
//!
//! 架构（plugins.md §五）：读 `<数据根>/mcp.json` → spawn MCP server
//! 子进程（stdio，newline-delimited JSON-RPC）→ initialize 握手 →
//! tools/list → 把每个 MCP 工具适配成本地 ToolSpec（`mcp_<server>_<tool>`，
//! schema 原样透传给模型）。调用时 JSON-RPC → MCP server → 投影返回。
//!
//! 安全模型：
//! - **mcp.json 是用户手写配置，模型不可达**——本插件不提供改配置的
//!   工具，spawn 什么进程完全由用户决定
//! - **每个工具的 side_effect 由配置声明，不写默认 Irreversible**
//!   （§五 安全兜底）；`compensable` 配置值按宁严勿松收紧为
//!   Irreversible（远端工具没有机械 undo）
//! - 工具名清洗（`[a-zA-Z0-9_-]` 之外替换 `_`）+ server 名禁下划线
//!   保证 `mcp_<server>_<tool>` 路由无歧义（全名即 OpenAI 兼容接口
//!   要求的 `^[a-zA-Z0-9_-]+$` 字符集）
//! - 内容上限 1MiB；握手 15s、调用超时按配置（缺省 30s 上限 120s）
//!
//! 生命周期：describe 时全量发现并**冻结工具清单**进 manifest；连接由
//! describe 与 make_state 共享（static 转交），死连接按调用懒重连；
//! mcp_reload 只重连不增删工具（清单变更需重载插件）。Drop 杀全部子进程。
//!
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。

mod config;
mod protocol;
mod registry;
#[cfg(test)]
mod tests;

use std::ffi::CString;
use std::sync::Arc;

use plugin_api::ffi::log_level;
use plugin_api::manifest::{PluginKind, PluginManifest};
use plugin_api::meta::official_meta;
use plugin_api::plugin_version;
use plugin_api::serde_json::{Value, json};
use plugin_api::tool::{SideEffect, ToolSpec};
use plugin_api::{HostVtable, PluginState};

use crate::registry::Registry;

const PLUGIN_ID: &str = "tool-mcp";

/// 管理工具（固定）：status 只读；reload 重连（工具清单冻结于加载时）。
fn management_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "mcp_status".into(),
            description: "List the configured MCP servers: connection state, tool count \
                          and the last connection error (if any), plus config-level \
                          errors. Pure."
                .into(),
            schema: json!({ "type": "object", "properties": {} }),
            side_effect: SideEffect::Pure,
            exposure: Default::default(),
        },
        ToolSpec {
            name: "mcp_reload".into(),
            description: "Re-read the MCP config and re-establish every server \
                          connection. The tool list was frozen when the plugin loaded — \
                          servers added to mcp.json afterwards only appear after \
                          reloading the plugin. Pure."
                .into(),
            schema: json!({ "type": "object", "properties": {} }),
            side_effect: SideEffect::Pure,
            exposure: Default::default(),
        },
    ]
}

/// describe 符号的数据源：管理工具 + mcp.json 发现出的全部远端工具
/// （NUL 结尾 JSON，约定见 plugin-api::ffi）。
fn describe_manifest() -> CString {
    let registry = registry::take_or_discover();
    registry::stash(Arc::clone(&registry));
    let mut tools = management_specs();
    tools.extend(registry.tool_specs());
    let mut manifest = PluginManifest {
        id: PLUGIN_ID.into(),
        version: plugin_version!().into(),
        abi_version: plugin_api::PLUGIN_ABI_VERSION,
        kind: PluginKind::Tool,
        tools,
        // 展示元数据（display_name / description / icon / author /
        // origin / homepage / license）由 plugin_api::meta 单点给出（审计 F7）。
        ..Default::default()
    };
    // 展示文案**不写"本次发现 N 个"**（原形状是 `&format!` 把运行期计数拼进去）：
    // 台账 T11 之后代码侧 describe 与磁盘 `manifest.json` 必须同源，而这个数是每次装载
    // 都变的快照，钉进清单就永远写着一个过期的数。审计批 F-21c 当年把它登记成"设计使然
    // 的例外"，前提是展示文案有消费者——实测没有：UI 读磁盘清单，`plugin-host` 只用
    // `.tools`，而"发现了几个"本来就由 `mcp_status` 这个管理工具报。所以这里收成静态句。
    official_meta(
        &mut manifest,
        ("MCP 桥接", "MCP Bridge"),
        (
            "桥接 MCP 生态：远端工具在加载时从 <数据根>/mcp.json 发现并冻结进工具清单（此处只列管理工具）",
            "Bridges the MCP ecosystem: remote tools are discovered from <data root>/mcp.json at load time and frozen into the catalog (only management tools listed here)",
        ),
    );
    manifest.to_describe_json()
}

struct McpState {
    host: *const HostVtable,
    registry: std::sync::Arc<Registry>,
}

// SAFETY: host 指向宿主的 'static vtable（只读）；registry 内部自带锁，
// 连接共享跨线程安全。init 之后 host 指针不再改变。
unsafe impl Send for McpState {}
unsafe impl Sync for McpState {}

impl McpState {
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

impl PluginState for McpState {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        self.log(log_level::INFO, &format!("tool-mcp: {tool} called"));
        match tool {
            "mcp_status" => Ok(json!({
                "servers": self.registry.status(),
                "config_errors": self.registry.cfg_errors,
            })),
            "mcp_reload" => {
                self.registry.reload();
                Ok(json!({
                    "reloaded": true,
                    "servers": self.registry.status(),
                }))
            }
            _ => {
                let rest = tool.strip_prefix("mcp_").ok_or_else(|| {
                    format!("unknown tool `{tool}` (expected mcp_<server>_<tool>)")
                })?;
                let (server, name) = rest.split_once('_').ok_or_else(|| {
                    format!("unknown tool `{tool}` (expected mcp_<server>_<tool>)")
                })?;
                self.registry.call(server, name, args)
            }
        }
    }
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    let registry = registry::take_or_discover();
    Box::new(McpState { host, registry })
}

plugin_api::declare_plugin!(describe_manifest, make_state);
