//! OS 外壳三件事：跨平台"打开目录"的共用壳 `open_in_explorer`（`commands/assets.rs`
//! 与 `commands/memory.rs` 都调它 ⇒ 保持 `pub`）、受 fsRoots 白名单约束的
//! `reveal_in_explorer`，与只放行 https / 无空白控制字符 / 限长的 `open_external_url`。
//! ⚠ 判定表登记的「shell 三分支两份」就在本片：`open_in_explorer` 与 `open_external_url`
//! 各抄一遍同一套 `#[cfg]` 三分法。本刀是纯搬行，**不夹带**合并——并成一处另开一刀。

use tauri::State;

use crate::state::AppState;

/// 跨平台打开目录（reveal_in_explorer / memory_reveal_folder 共用壳）。
pub fn open_in_explorer(p: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(p)
            .spawn()
            .map_err(|e| format!("打开文件管理器失败: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(p)
            .spawn()
            .map_err(|e| format!("打开文件管理器失败: {e}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(p)
            .spawn()
            .map_err(|e| format!("打开文件管理器失败: {e}"))?;
    }
    Ok(())
}

/// 在系统文件管理器中打开项目目录（§2.3 v3.9 项目菜单「在文件夹中显示」）：
/// 路径必须是当前 fsRoots 之一（后端直读设置校验，不信前端转述——与
/// `session_associate` 同一防线），不给任意路径开执行面。
#[tauri::command]
pub async fn reveal_in_explorer(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let p = path.trim();
    let roots = state.settings.fs_roots();
    if !roots.iter().any(|r| r == p) {
        return Err(format!("「{p}」不在 设置 → 工具 → 工作区目录 里，拒绝打开"));
    }
    open_in_explorer(p)
}

/// 打开外部网页链接（批 A「如何获取 API Key ↗」= 预设供应商文档）：只放
/// 行 https、无空白/控制字符、限长——不给任意协议/命令注入开执行面；
/// shell 套路同 `open_in_explorer`（explorer / open / xdg-open）。
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    let u = url.trim();
    if !u.starts_with("https://") {
        return Err("只允许打开 https 链接".into());
    }
    if u.len() > 500 || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("链接形状不合法，拒绝打开".into());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(u)
            .spawn()
            .map_err(|e| format!("打开链接失败: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(u)
            .spawn()
            .map_err(|e| format!("打开链接失败: {e}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(u)
            .spawn()
            .map_err(|e| format!("打开链接失败: {e}"))?;
    }
    Ok(())
}
