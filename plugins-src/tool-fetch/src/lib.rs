//! tool-fetch：第一批真实工具插件（plugins.md P0，阶段八③落地）。
//!
//! 一个工具 `fetch`（HTTP GET / 网页抓取，`SideEffect::Irreversible`——
//! 2026-10-06 真实测试复测裁定：GET 也对外发起网络请求（服务端留痕、
//! URL 可能带敏感 token），"只读"标法与事实不符；plugins.md 同步改）。
//!
//! 行为约定：
//! - 只接受 http / https（`file:` 等协议直接拒绝）
//! - **SSRF 防线**（用户 2026-09-29 指定）：内网 / 环回 / 链路本地地址
//!   一律拒绝（`ssrf.rs`）；重定向**不跟随**（302 → 内网是绕过防线的
//!   最短路径）；`LOCOWRK_FETCH_ALLOW_PRIVATE=1` 显式放开（本机开发）
//! - 响应体超过 max_bytes 截断（防一次抓取灌爆模型上下文）
//! - 4xx / 5xx **不报错**：状态码与响应正文一起返回——对模型来说
//!   "404 页面说了什么"通常比"调用失败"更有用；传输层错误（DNS /
//!   超时 / 连接拒绝）才返回 Err
//! - 默认超时 10s（上限 60s）
//!
//! 阻塞式 ureq 跑在宿主的 spawn_blocking 里（FFI 纪律三，§5.6）。
//! 版本号来自编译期环境变量 LOCOWRK_PLUGIN_VERSION（见 build.rs）。

mod ssrf;

use std::collections::BTreeMap;
use std::ffi::CString;
use std::io::Read;
use std::time::Duration;

use plugin_api::args::arg_u64;
use plugin_api::ffi::log_level;
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

const PLUGIN_ID: &str = "tool-fetch";

/// 响应体缺省/上限（字节）。
const BODY_DEFAULT: u64 = 256 * 1024;
const BODY_MAX: u64 = 1024 * 1024;
/// 请求超时缺省/上限（秒）。
const TIMEOUT_DEFAULT: u64 = 10;
const TIMEOUT_MAX: u64 = 60;

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
            name: "fetch".into(),
            description: "Fetch a URL over HTTP GET and return the response body as text. \
                          Only http/https. Returns {url, status, content_type, truncated, body}; \
                          body is capped at max_bytes (default 256KiB). HTTP error statuses \
                          (4xx/5xx) are returned as data, not failures. Private/loopback/link-local hosts are refused by the SSRF guard and redirects are not followed. \
                          Performs an outbound network request to the target host (the visit \
                          shows up in its server logs; the URL may carry sensitive tokens) — \
                          classified Irreversible, so approval follows the session's \
                          permission mode; it is not a read-only lookup."
                .into(),
            schema: json!({
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "absolute http(s) URL" },
                    "max_bytes": { "type": "number",
                                   "description": "cap returned body (default 262144, max 1048576)" },
                    "timeout_secs": { "type": "number",
                                      "description": "request timeout (default 10, max 60)" }
                },
                "required": ["url"]
            }),
            side_effect: SideEffect::Irreversible,
            exposure: Default::default(),
        }],
        // v2 展示元数据（packaging.md §2.2；仓库自带插件显式声明 official）
        display_name: Some(zh_en("网页抓取", "Web Fetch")),
        description: Some(LocalizedOrString::Localized(zh_en(
            "抓取 HTTP(S) 页面正文（限长、限超时）",
            "Fetch an HTTP(S) page body (size-capped, timeout-capped)",
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

struct FetchState {
    host: *const HostVtable,
}

// SAFETY: host 指向宿主的 'static vtable（只读），FetchState 自身无可变共享
// 状态；init 之后指针不再改变，跨线程共享安全。
unsafe impl Send for FetchState {}
unsafe impl Sync for FetchState {}

impl FetchState {
    fn log(&self, level: u32, msg: &str) {
        // SAFETY: host 是 init 时宿主交付的 'static vtable 指针
        unsafe {
            if let Some(vt) = self.host.as_ref() {
                vt.log_str(level, msg);
            }
        }
    }
}

impl PluginState for FetchState {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        if tool != "fetch" {
            return Err(format!("unknown tool `{tool}`"));
        }
        self.log(log_level::INFO, "tool-fetch: fetch called");
        fetch(&args)
    }
}

fn fetch(args: &Value) -> Result<Value, String> {
    let url = args
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `url`".to_string())?;
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err("fetch only supports http:// and https:// URLs".into());
    }
    // SSRF 防线：内网/环回/链路本地拒绝（IP 字面量直判，域名解析后逐 IP 判）
    ssrf::SSRF.guard(url)?;
    let max = arg_u64(args, "max_bytes", BODY_DEFAULT, BODY_MAX)?;
    let timeout = arg_u64(args, "timeout_secs", TIMEOUT_DEFAULT, TIMEOUT_MAX)?;

    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout))
        // 重定向不跟随：302 → 内网是绕过 SSRF 防线的最短路径；3xx 走
        // Status 错误路径 → 状态码与正文照常返回给模型
        .redirects(0)
        .build();
    // ureq 对 4xx/5xx 返回 Error::Status(code, response)：正文照读、状态码
    // 进返回值（见模块注释）；只有传输层错误才变成 Err。
    let resp = match agent.get(url).call() {
        Ok(resp) => resp,
        Err(ureq::Error::Status(_code, resp)) => resp,
        Err(e) => return Err(format!("fetch `{url}`: {e}")),
    };

    let status = resp.status();
    let content_type = resp.content_type().to_string();
    let final_url = resp.get_url().to_string();
    let mut buf = Vec::new();
    resp.into_reader()
        .take(max + 1)
        .read_to_end(&mut buf)
        .map_err(|e| format!("fetch `{url}`: read body: {e}"))?;
    let truncated = buf.len() as u64 > max;
    buf.truncate(max as usize);
    Ok(json!({
        "url": final_url,
        "status": status,
        "content_type": content_type,
        "truncated": truncated,
        "body": String::from_utf8_lossy(&buf).as_ref(),
    }))
}

fn make_state(host: *const HostVtable) -> Box<dyn PluginState> {
    Box::new(FetchState { host })
}

plugin_api::declare_plugin!(describe_manifest, make_state);
