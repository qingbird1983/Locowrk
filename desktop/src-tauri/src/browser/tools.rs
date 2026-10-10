//! 浏览器面板的模型工具面（v2.144）：`browser_open`（把工作区 HTML/图片
//! 或 http(s) 页面打开进工作面板浏览器页签——模型交付可运行成品的正门，
//! 替代即兴 `<img src>`）+ `browser_console`（读回页面 console 报错，验
//! 证环）。双件都 Pure + OnDemand：纯展示面，无宿主副作用不走审批。

use std::sync::Arc;

use async_trait::async_trait;
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{json, Value};

use super::console::ConsoleStore;
use super::preview;
use crate::settings::SettingsStore;

/// UI 推送通道（前端 work-panel store 订阅开页签）。
pub const CHANNEL_OPEN: &str = "lw://browser-open";

/// 事件出口（组装点注入 AppHandle emit 闭包，v2.66 同款：AppHandle 不进模块）。
pub type EmitFn = Arc<dyn Fn(&str, Value) + Send + Sync>;

fn failed(tool: &str, message: impl Into<String>) -> ToolError {
    ToolError::Failed {
        tool: tool.into(),
        message: message.into(),
    }
}

/// 可作为页签入口的扩展名（页面渲染型；页内子资源不受此限——协议层只认
/// fsRoots 围栏）。
const ENTRY_EXTENSIONS: &[&str] = &[
    "html", "htm", "svg", "png", "jpg", "jpeg", "gif", "webp", "avif", "bmp", "ico", "txt", "md",
    "json",
];

fn valid_url(u: &str) -> Result<(), String> {
    if !(u.starts_with("http://") || u.starts_with("https://")) {
        return Err("url must start with http:// or https://".into());
    }
    if u.len() > 500 || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("url shape invalid".into());
    }
    Ok(())
}

// ---- browser_open ----

fn open_spec() -> &'static ToolSpec {
    static SPEC: std::sync::OnceLock<ToolSpec> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "browser_open".into(),
        description: "Open a page in the work-panel browser tab (embedded preview) so the user \
                      can see and interact with it. Pass EITHER `path` (absolute path of a \
                      renderable file — html/htm/svg/images/txt/md/json — inside a configured \
                      workspace root; use this to show the user a generated HTML page such as a \
                      game) OR `url` (http/https only). Optional `title`. After opening, page \
                      console errors/warnings are reported to the `browser_console` tool — \
                      call it to verify the page actually runs. Pure display, no approval."
            .into(),
        schema: json!({
            "type": "object",
            "properties": {
                "path": { "type": "string",
                          "description": "absolute path of a renderable file inside a workspace root" },
                "url": { "type": "string",
                         "description": "http(s) URL to preview" },
                "title": { "type": "string",
                           "description": "optional tab title (default: file name / host)" }
            }
        }),
        side_effect: SideEffect::Pure,
        exposure: Exposure::OnDemand,
    })
}

pub struct BrowserOpenTool {
    settings: Arc<SettingsStore>,
    console: Arc<ConsoleStore>,
    emit: EmitFn,
}

impl BrowserOpenTool {
    pub(crate) fn new(
        settings: Arc<SettingsStore>,
        console: Arc<ConsoleStore>,
        emit: EmitFn,
    ) -> Arc<Self> {
        Arc::new(Self {
            settings,
            console,
            emit,
        })
    }
}

#[async_trait]
impl Tool for BrowserOpenTool {
    fn spec(&self) -> &ToolSpec {
        open_spec()
    }

    fn plugin_version(&self) -> &semver::Version {
        static V: std::sync::LazyLock<semver::Version> =
            std::sync::LazyLock::new(|| semver::Version::new(1, 0, 0));
        &V
    }

    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let path = args.get("path").and_then(|v| v.as_str()).map(str::trim);
        let url = args.get("url").and_then(|v| v.as_str()).map(str::trim);
        let title_in = args
            .get("title")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty());

        let (kind, final_url, target, title) = match (path, url) {
            (Some(p), None) if !p.is_empty() => {
                if !std::path::Path::new(p).is_absolute() {
                    // 报错写给模型看，示例必须跟宿主平台一致（本 crate
                    // 三平台构建；旧文案把 Windows 路径写死，非 Windows 下
                    // 模型会照抄一个非法示例）。
                    let example = if cfg!(windows) {
                        r#"e.g. F:\dir\page.html"#
                    } else {
                        "e.g. /home/you/page.html"
                    };
                    return Err(failed(
                        "browser_open",
                        format!("path must be absolute ({example})"),
                    ));
                }
                let ext_ok = std::path::Path::new(p)
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| ENTRY_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
                    .unwrap_or(false);
                if !ext_ok {
                    return Err(failed(
                        "browser_open",
                        format!(
                            "entry file must be one of [{}]; page assets (js/css) are loaded by the page itself",
                            ENTRY_EXTENSIONS.join(", ")
                        ),
                    ));
                }
                // fsRoots 围栏：与协议处理器同一判定（canonicalize 前缀比对）
                let roots = self.settings.fs_roots();
                let canonical = preview::scope_check(&roots, std::path::Path::new(p))
                    .map_err(|status| {
                        failed(
                            "browser_open",
                            if status == 403 {
                                "path is outside the configured workspace roots (设置 → 工具 → 工作区目录)".into()
                            } else {
                                format!("file not found: {p}")
                            },
                        )
                    })?;
                let name = canonical
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("page")
                    .to_string();
                (
                    "file",
                    preview::preview_url(&canonical.to_string_lossy()),
                    p.to_string(),
                    title_in.unwrap_or(&name).to_string(),
                )
            }
            (None, Some(u)) if !u.is_empty() => {
                valid_url(u).map_err(|e| failed("browser_open", e))?;
                let host = u
                    .split("://")
                    .nth(1)
                    .and_then(|rest| rest.split(['/']).next())
                    .unwrap_or("page")
                    .to_string();
                (
                    "url",
                    u.to_string(),
                    u.to_string(),
                    title_in.unwrap_or(&host).to_string(),
                )
            }
            _ => {
                return Err(failed(
                    "browser_open",
                    "pass exactly one of `path` or `url`",
                ))
            }
        };

        let tab_id = self.console.next_tab_id();
        self.console.register(&tab_id);
        let payload = json!({
            "tabId": tab_id,
            "kind": kind,
            "url": final_url,
            "target": target,
            "title": title,
        });
        (self.emit)(CHANNEL_OPEN, payload.clone());
        Ok(payload)
    }
}

// ---- browser_console ----

fn console_spec() -> &'static ToolSpec {
    static SPEC: std::sync::OnceLock<ToolSpec> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "browser_console".into(),
        description: "Read recent console errors/warnings collected from the work-panel browser \
                      page (an injected bridge reports console.error/warn, window errors and \
                      unhandled rejections). Optional `tabId` (default = most recent tab). Use \
                      after `browser_open` (and after the user has interacted) to verify the \
                      page runs clean; empty result = no errors reported so far, not proof of \
                      correctness."
            .into(),
        schema: json!({
            "type": "object",
            "properties": {
                "tabId": { "type": "string",
                           "description": "tab id returned by browser_open (default: latest)" }
            }
        }),
        side_effect: SideEffect::Pure,
        exposure: Exposure::OnDemand,
    })
}

pub struct BrowserConsoleTool {
    console: Arc<ConsoleStore>,
}

impl BrowserConsoleTool {
    pub(crate) fn new(console: Arc<ConsoleStore>) -> Arc<Self> {
        Arc::new(Self { console })
    }
}

#[async_trait]
impl Tool for BrowserConsoleTool {
    fn spec(&self) -> &ToolSpec {
        console_spec()
    }

    fn plugin_version(&self) -> &semver::Version {
        static V: std::sync::LazyLock<semver::Version> =
            std::sync::LazyLock::new(|| semver::Version::new(1, 0, 0));
        &V
    }

    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let tab = args.get("tabId").and_then(|v| v.as_str()).map(str::trim);
        match self.console.read(tab) {
            Some((id, entries, truncated)) => Ok(json!({
                "tabId": id,
                "entries": entries,
                "truncated": truncated,
            })),
            None => Ok(json!({
                "tabId": tab,
                "entries": [],
                "truncated": false,
                "note": "no browser tab has reported console output yet (open one with browser_open)",
            })),
        }
    }
}

#[cfg(test)]
mod tests;
