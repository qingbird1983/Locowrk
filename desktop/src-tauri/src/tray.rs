//! 系统托盘与全局快捷键（DESIGN.md §14 阶段八⑤ / §13.1 快捷键）。
//!
//! - 托盘：左键单击 = 显示并聚焦主窗口；菜单 = 显示主窗口 / 退出。
//! - 主窗口关闭 = 隐藏进托盘（CloseRequested 拦截）；真退出只走托盘菜单。
//! - 全局快捷键：settings.json `shortcut`（缺省 `Ctrl+Shift+L`），按下 =
//!   切换主窗口可见性；设置面板改键后 `apply_shortcut` 即时重注册。
//!
//! 纪律：本模块只操作窗口与快捷键注册，不写业务逻辑；键位字符串校验在
//! settings.rs（`parse::<Shortcut>`），本模块只负责注册成败。

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

use crate::state::AppState;

/// global-shortcut 插件（统一 handler：任何已注册快捷键按下 = 切窗）。
/// 必须在 `.setup()` 之前挂上（插件先于 setup 初始化）。
pub fn plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                toggle_main(app);
            }
        })
        .build()
}

/// 组装托盘 + 注册全局快捷键。在 `app.manage(state)` 之后调用（要读
/// settings 里已存的键位）。
pub fn setup<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    build_tray(app)?;
    hide_on_close(app);
    // 注册失败（键位被其他程序占用等）不致命：降级为无快捷键启动，
    // 设置面板换一个键即可即时重试。
    let shortcut = app.state::<AppState>().settings.shortcut();
    if let Err(e) = apply_shortcut(app, &shortcut) {
        tracing::warn!("全局快捷键注册失败（无快捷键启动）: {e}");
    }
    Ok(())
}

fn build_tray<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    let show = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;
    let mut tray = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}

/// 拦截主窗口关闭 → 隐藏进托盘（托盘菜单「退出」才是真退出）。
fn hide_on_close<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let closer = win.clone();
        win.on_window_event(move |event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = closer.hide();
            }
        });
    }
}

/// 全局快捷键动作：切换主窗口可见性（可见 → 隐藏，否则 → 显示并聚焦）。
pub fn toggle_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        if win.is_visible().unwrap_or(false) {
            let _ = win.hide();
        } else {
            let _ = win.show();
            let _ = win.set_focus();
        }
    }
}

fn show_main<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}

/// 注册（或改注册）全局快捷键：先挂新键（失败 → 旧键原样保留），成功
/// 后清场重挂，恒保持「至多一个全局键」。
pub fn apply_shortcut<R: Runtime>(app: &AppHandle<R>, new: &str) -> Result<(), String> {
    let parsed: Shortcut = new
        .parse()
        .map_err(|e| format!("无法识别的快捷键 {new}: {e}"))?;
    let gs = app.global_shortcut();
    gs.register(parsed)
        .map_err(|e| format!("快捷键 {new} 注册失败: {e}（可能已被其他程序占用）"))?;
    let _ = gs.unregister_all();
    gs.register(parsed)
        .map_err(|e| format!("快捷键 {new} 重挂失败: {e}"))?;
    Ok(())
}
