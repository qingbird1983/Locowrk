//! OS 外壳三件事：跨平台"打开目录"的共用壳 `open_in_explorer`（`commands/assets.rs`
//! 调它 ⇒ 保持 `pub`）、受 fsRoots 白名单约束的
//! `reveal_in_explorer`，与只放行 https / 无空白控制字符 / 限长的 `open_external_url`。
//! 判定表登记的「shell 三分支两份」已随 T28 并成一处：explorer / open / xdg-open
//! 三分法只写在私有壳 `shell_open` 里，两条命令各带自己的失败标签。

use tauri::State;

use crate::state::AppState;

/// 跨平台 spawn 打开的共用壳（T28 并处）：explorer / open / xdg-open 三分法
/// **只写这一遍**。失败标签由调用方带（「打开文件管理器失败」/「打开链接失败」，
/// 用户可见文案不漂；标签在 spawn 面测不进，靠调用点字面量 + 真窗口目检兜底）。
fn shell_open(target: &str, fail_label: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(target)
            .spawn()
            .map_err(|e| format!("{fail_label}: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("{fail_label}: {e}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(target)
            .spawn()
            .map_err(|e| format!("{fail_label}: {e}"))?;
    }
    Ok(())
}

/// 跨平台打开目录（reveal_in_explorer / assets.rs 共用壳）。
pub fn open_in_explorer(p: &str) -> Result<(), String> {
    shell_open(p, "打开文件管理器失败")
}

/// 在系统文件管理器中打开项目目录（§2.3 v3.9 项目菜单「在文件夹中显示」）：
/// 路径必须是当前 fsRoots 之一（后端直读设置校验，不信前端转述——与
/// `session_associate` 同一条防线 `meta::ensure_in_fs_roots`，T28 并处），
/// 不给任意路径开执行面。
#[tauri::command]
pub async fn reveal_in_explorer(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let p = path.trim();
    super::meta::ensure_in_fs_roots(&state.settings.fs_roots(), p, "拒绝打开")?;
    open_in_explorer(p)
}

/// 打开外部网页链接（批 A「如何获取 API Key ↗」= 预设供应商文档）：只放
/// 行 https、无空白/控制字符、限长——不给任意协议/命令注入开执行面；
/// spawn 套路走共用壳 `shell_open`（explorer / open / xdg-open 三分法一处）。
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    let u = url.trim();
    if !u.starts_with("https://") {
        return Err("只允许打开 https 链接".into());
    }
    if u.len() > 500 || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("链接形状不合法，拒绝打开".into());
    }
    shell_open(u, "打开链接失败")
}
