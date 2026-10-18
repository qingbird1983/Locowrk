//! OS 原生对话框三个出口（§七 规则 8 同族：tauri-plugin-dialog 仅 Rust 侧调用，
//! 无 JS guest、无需 capability）。取消 = None / 空表，是否采纳由调用分区决定。

use tauri::AppHandle;

// ---- 原生目录选择（⑱，UI shell 文档 §7 规则 8）：tauri-plugin-dialog 仅
// Rust 侧调用（同 notification 先例：无 JS guest、无需 capability）。async
// 命令跑在异步运行时线程而非主线程，blocking API 安全。取消/关窗 = None，
// 是否采纳由调用分区决定。 ----

/// 打开 OS 原生目录选择对话框，返回所选目录绝对路径（取消返回 None）。
#[tauri::command]
pub async fn pick_directory(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app.dialog().file().blocking_pick_folder();
    Ok(picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.display().to_string()))
}

/// 打开 OS 原生存储对话框（v2.49 会话导出用，§七 规则 8 同族：仅 Rust
/// 侧调用）：`default_name` 建议文件名（空 = 不预设），返回所选绝对路径
/// （取消返回 None）。
#[tauri::command]
pub async fn pick_save_path(
    default_name: String,
    app: AppHandle,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut builder = app.dialog().file();
    let name = default_name.trim();
    if !name.is_empty() {
        builder = builder.set_file_name(name);
    }
    let picked = builder.blocking_save_file();
    Ok(picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.display().to_string()))
}

/// 原生多选文件对话框（§3.11 v3.9 composer「＋添加文件或图片」）：仅
/// Rust 侧调用（§七 规则 8 同族），返回所选绝对路径数组（取消 = 空表）。
/// 路径由前端以 `@路径` 引用写进输入框——附件随消息上传的消息通路属
/// ㉛ 排期，当前模型经 fs 工具按路径读取（v1 语义，宁缺勿假）。
#[tauri::command]
pub async fn pick_files(app: AppHandle) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let picked = app.dialog().file().blocking_pick_files();
    let mut out = Vec::new();
    for p in picked.into_iter().flatten() {
        if let Ok(path) = p.into_path() {
            out.push(path.display().to_string());
        }
    }
    Ok(out)
}
