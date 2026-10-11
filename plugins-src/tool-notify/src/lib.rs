//! tool-notify：消息推送（plugins.md P2，⑭ 落地）。
//!
//! 一个工具 `notify`，通道按参数二选一：
//! - `desktop`：桌面系统通知。插件不经网络不经文件——直接调宿主 vtable
//!   **末尾追加的 notify 槽**（append-only，`PLUGIN_ABI_VERSION` 不动；
//!   headless 宿主返回 UNSUPPORTED，这里如实报错给模型，不静默吞）。
//! - `webhook`：HTTP(S) POST JSON 到用户给的端点。技术栈与 SSRF 防线
//!   照搬 tool-fetch（`ssrf.rs`：目标解析固定 IP、私网段拒绝、重定向
//!   不跟随——v2.14 防线纪律不复核）；非 2xx = 未送达 = 报错（与 fetch
//!   把 4xx 当数据不同：通知的语义是"送达与否"，含糊不得）。
//!
//! 工具整体标 Irreversible（宁严勿松）：webhook 是对外发数据，desktop
//! 虽无害但分级是工具级粒度——审批一次不冤枉。阻塞式 ureq 跑在宿主的
//! spawn_blocking 里（FFI 纪律三，§5.6）。版本号来自编译期环境变量
//! LOCOWRK_PLUGIN_VERSION（见 build.rs 同 tool-fetch 的约定）。

mod ssrf;

use std::collections::BTreeMap;
use std::ffi::CString;
use std::io::Read;
use std::time::Duration;

use plugin_api::args::arg_u64;
use plugin_api::ffi::{code, log_level};
use plugin_api::manifest::{
    Author, LocalizedOrString, LocalizedString, PluginKind, PluginManifest, PluginOrigin,
};
use plugin_api::serde_json::{Value, json};
use plugin_api::tool::{SideEffect, ToolSpec};
use plugin_api::{HostVtable, PluginState};

/// 双语展示元数据样板（packaging.md §2.2）。
fn zh_en(zh: &str, en: &str) -> LocalizedString {
    LocalizedString(BTreeMap::from([
        ("zh-CN".to_owned(), zh.to_owned()),
        ("en-US".to_owned(), en.to_owned()),
    ]))
}

const PLUGIN_ID: &str = "tool-notify";

/// webhook 响应体读取上限（不进返回值，只为释放连接不拖死对端）。
const RESPONSE_DRAIN: u64 = 4096;
/// 请求超时缺省/上限（秒）。
const TIMEOUT_DEFAULT: u64 = 10;
const TIMEOUT_MAX: u64 = 60;
/// 文案长度上限：通知与 webhook 载荷都不该是一次性灌爆的东西。
const TEXT_MAX: usize = 2000;

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
            name: "notify".into(),
            description: "Send a notification with title + body. Channels (exactly one): \
                          `channel: \"desktop\"` shows a system notification on this machine; \
                          `channel: \"webhook\"` POSTs {title, body, source} as JSON to `url` \
                          (http/https only; private/loopback addresses refused by the SSRF \
                          guard, redirects not followed; non-2xx counts as failed delivery). \
                          Requires approval (webhook sends data off-machine)."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "channel": { "type": "string",
                                 "enum": ["desktop", "webhook"],
                                 "description": "where to deliver" },
                    "title": { "type": "string", "description": "short headline (max 200 chars)" },
                    "body": { "type": "string", "description": "message content (max 2000 chars)" },
                    "url": { "type": "string",
                             "description": "absolute http(s) URL; required for webhook channel" },
                    "timeout_secs": { "type": "number",
                                      "description": "webhook request timeout (default 10, max 60)" }
                },
                "required": ["channel", "title", "body"]
            }),
            side_effect: SideEffect::Irreversible,
            exposure: Default::default(),
        }],
        // v2 展示元数据（packaging.md §2.2；仓库自带插件显式声明 official）
        display_name: Some(zh_en("消息推送", "Push Notify")),
        description: Some(LocalizedOrString::Localized(zh_en(
            "桌面系统通知 / Webhook 推送（SSRF 防线、限超时）",
            "Desktop notification / webhook push (SSRF-guarded, timeout-capped)",
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

struct NotifyState {
    host: *const HostVtable,
}

// SAFETY: host 指向宿主的 'static vtable（只读），NotifyState 自身无可变
// 共享状态；init 之后指针不再改变，跨线程共享安全。
unsafe impl Send for NotifyState {}
unsafe impl Sync for NotifyState {}

impl NotifyState {
    fn log(&self, level: u32, msg: &str) {
        // SAFETY: host 是 init 时宿主交付的 'static vtable 指针
        unsafe {
            if let Some(vt) = self.host.as_ref() {
                vt.log_str(level, msg);
            }
        }
    }

    /// desktop 通道：经 vtable 追加槽投递系统通知。返回 Err 文本如实
    /// 描述失败（UNSUPPORTED = 宿主无桌面环境；其余 = 投递失败）。
    fn desktop(&self, title: &str, body: &str) -> Result<Value, String> {
        // SAFETY: host 是 init 时宿主交付的 'static vtable 指针
        let rc = unsafe {
            self.host
                .as_ref()
                .ok_or("host vtable unavailable")?
                .notify_str(title, body)
        };
        match rc {
            code::OK => Ok(json!({ "delivered": "desktop" })),
            code::ERR_UNSUPPORTED => Err(
                "desktop notifications are unavailable in this host (no desktop session)".into(),
            ),
            other => Err(format!("desktop notification failed (code {other})")),
        }
    }
}

impl PluginState for NotifyState {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        if tool != "notify" {
            return Err(format!("unknown tool `{tool}`"));
        }
        let channel = args
            .get("channel")
            .and_then(Value::as_str)
            .ok_or_else(|| "missing string field `channel` (desktop|webhook)".to_string())?;
        let title = text_arg(&args, "title", 200)?;
        let body = text_arg(&args, "body", TEXT_MAX)?;
        match channel {
            "desktop" => self.desktop(&title, &body),
            "webhook" => {
                self.log(log_level::INFO, "tool-notify: webhook called");
                webhook(&args, &title, &body)
            }
            other => Err(format!(
                "unknown channel `{other}` (expected \"desktop\" or \"webhook\")"
            )),
        }
    }
}

fn text_arg(args: &Value, key: &str, max_chars: usize) -> Result<String, String> {
    let s = args
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| format!("missing non-empty string field `{key}`"))?;
    let n = s.chars().count();
    if n > max_chars {
        return Err(format!("field `{key}` exceeds {max_chars} chars ({n})"));
    }
    Ok(s.to_owned())
}

fn webhook(args: &Value, title: &str, body: &str) -> Result<Value, String> {
    let url = args
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| "webhook channel requires string field `url`".to_string())?;
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("webhook only supports http:// and https:// URLs".into());
    }
    // SSRF 防线：内网/环回/链路本地拒绝（IP 字面量直判，域名解析后逐 IP 判）
    ssrf::SSRF.guard(url)?;
    let timeout = arg_u64(args, "timeout_secs", TIMEOUT_DEFAULT, TIMEOUT_MAX)?;

    let payload = json!({ "title": title, "body": body, "source": "locowrk" });
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout))
        // 重定向不跟随：302 → 内网是绕过 SSRF 防线的最短路径
        .redirects(0)
        .build();
    let resp = agent
        .post(url)
        .set("Content-Type", "application/json")
        .send_string(&payload.to_string())
        .map_err(|e| format!("webhook `{url}`: {e}"))?;
    let status = resp.status();
    // 通知的语义是"送达与否"：非 2xx = 未送达 = 报错（不学 fetch 把
    // 4xx 当数据——含糊的"成功"会骗模型谎报已通知）
    if !(200..300).contains(&status) {
        return Err(format!("webhook `{url}` responded {status}"));
    }
    let _ = resp
        .into_reader()
        .take(RESPONSE_DRAIN)
        .read_to_end(&mut Vec::new());
    Ok(json!({ "delivered": "webhook", "status": status }))
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    Box::new(NotifyState { host })
}

plugin_api::declare_plugin!(describe_manifest, make_state);
