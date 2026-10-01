//! 桌面系统通知（⑭）：两个通道共用这一个出口——
//! - tool-notify 的 desktop 通道：插件经宿主 vtable 追加槽打进来，
//!   plugin-host 把它转给 `set_notify_hook` 注入的闭包（本模块注册）；
//! - 调度引擎的兜底：触发时窗口不在位（不可见或未聚焦），聊天里的
//!   SystemNotice 用户看不到，补一条系统通知把人拉回来。
//!
//! 通知失败只 warn 不上抛（toast 是补充通道；"触发过"这件事以会话日志
//! 与 SystemNotice 为准——用户可能本就关了系统通知权限，缺一条不损真）。

use std::sync::Arc;

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::NotificationExt;

/// 发系统通知。失败返回 Err 文本（调用方决定 warn 还是回给模型——
/// 工具通道必须如实报错，兜底通道只记日志）。
pub fn show(app: &AppHandle, title: &str, body: &str) -> Result<(), String> {
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|e| format!("desktop notification: {e}"))
}

/// 窗口"在位"= 主窗口可见**且**聚焦。只可见不聚焦（用户在别的应用里）
/// 同样看不到会话内容，按在位处理会漏通知——比 ROADMAP 行文"窗口不可
/// 见"略收严，宁多勿漏。
pub(crate) fn window_attended(app: &AppHandle) -> bool {
    app.get_webview_window("main")
        .map(|w| {
            w.is_visible().unwrap_or(false) && w.is_focused().unwrap_or(false)
        })
        .unwrap_or(false)
}

/// 把系统通知实现注入 plugin-host 的 vtable 槽（init_state 时调一次；
/// 须在任何插件 init 之前，见 host_vtable 模块注释）。
pub fn register_vtable_hook(app: AppHandle) {
    plugin_host::set_notify_hook(Arc::new(move |title, body| {
        if let Err(e) = show(&app, title, body) {
            tracing::warn!("{e}");
        }
    }));
}
