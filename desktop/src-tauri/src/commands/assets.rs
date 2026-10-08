//! 资产命令（批 D，多模态设计 v2.0 §6/§9）：上传 / 缩略图读取 / 定位 /
//! 手动 GC。薄适配层——校验与存储全在 `crate::assets`（单测在 assets/ 模块）。

use base64::Engine;
use tauri::State;

use session_store::AssetRef;

use crate::assets::gc::GcReport;
use crate::state::AppState;

/// 上传即落盘（base64 通路，粘贴/拖拽的内存文件走这里）：载荷 + 申报
/// mime + 原文件名 → 内容寻址 `AssetRef`。校验（魔数白名单 / 体积 / 图片
/// 单边）在 [`crate::assets::validate`]，上限读 settings `multimodal` 区。
/// **上传门是第一道**（前端提示卡管选择）；模型不支持的第二道门在发送前
/// （wire）。
#[tauri::command]
pub async fn asset_put(
    data_base64: String,
    mime: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<AssetRef, String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_base64.trim())
        .map_err(|e| format!("附件数据不是合法 base64: {e}"))?;
    if bytes.is_empty() {
        return Err("附件内容为空".into());
    }
    let limits = upload_limits(&state);
    state.assets.put(&bytes, &mime, &name, &limits)
}

/// 上传即落盘（路径通路，原生对话框选中的文件走这里——零 IPC 大载荷）。
/// 用户经系统对话框显式选中 = 读取授权；落点只在本数据根 assets/ 的
/// sha256 文件名内，无任意路径执行面。原文件名取路径尾段（展示用）。
#[tauri::command]
pub async fn asset_put_path(path: String, state: State<'_, AppState>) -> Result<AssetRef, String> {
    let p = path.trim().to_string();
    let (bytes, name) = tokio::task::spawn_blocking(move || {
        let bytes = std::fs::read(&p).map_err(|e| format!("文件读取失败: {e}"))?;
        let name = std::path::Path::new(&p)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("file")
            .to_string();
        Ok::<_, String>((bytes, name))
    })
    .await
    .map_err(|e| format!("读取任务失败: {e}"))??;
    if bytes.is_empty() {
        return Err("文件内容为空".into());
    }
    let limits = upload_limits(&state);
    state.assets.put(&bytes, "", &name, &limits)
}

/// 缩略图读取（MessageRow 渲染）：mime + base64 载荷（前端拼 data URL，
/// 自带内存缓存）。悬空引用如实报错。
#[tauri::command]
pub async fn asset_read(
    id: String,
    mime: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    state.assets.data_url(&id, &mime)
}

/// 缩略图点击「在文件夹中显示」（批 D 大图查看器的系统打开器顶位）：
/// id + mime → assets 目录内路径（sha256 文件名，无路径穿越面）。
#[tauri::command]
pub async fn asset_reveal(id: String, mime: String, state: State<'_, AppState>) -> Result<(), String> {
    let path = state.assets.path_of(id.trim(), mime.trim())?;
    if !path.exists() {
        return Err(format!("资产文件不存在: {}", path.display()));
    }
    crate::commands::sessions::open_in_explorer(&path.display().to_string())
}

/// 手动 GC（走查/开发者入口）：按 settings `gcThresholdDays` 判定孤儿，
/// 返回计数如实上报。autoGc 关闭也可手动触发。
#[tauri::command]
pub async fn asset_gc(state: State<'_, AppState>) -> Result<GcReport, String> {
    let days = state.settings.multimodal().gc_threshold_days;
    let root = state.assets.root().to_path_buf();
    let sessions = state.sessions_dir.clone();
    let subs = state.sub_transcripts_dir.clone();
    let report = tokio::task::spawn_blocking(move || {
        crate::assets::gc::run_gc(&root, &sessions, &subs, days)
    })
    .await
    .map_err(|e| format!("GC 任务失败: {e}"))?;
    tracing::info!(scanned = report.scanned, removed = report.removed, "asset gc done");
    Ok(report)
}

fn upload_limits(state: &State<'_, AppState>) -> crate::assets::validate::Limits {
    let m = state.settings.multimodal();
    crate::assets::validate::Limits {
        max_dimension: m.max_dimension,
        max_size_mb: m.max_size_mb,
        image_formats: m.image_formats,
        file_formats: m.file_formats,
    }
}

/// 查看器「另存为」（批 E 查看器）：原生保存对话框（Rust 侧直呼，§七 规
/// 则 8 同族）→ 资产文件复制到选定路径。取消 = None。读取面只认 assets/
/// 内 sha256 文件名（id 定位，无路径穿越面）；写入位置由用户对话框显式
/// 选定（与 session_export 同授权形态）。
#[tauri::command]
pub async fn asset_save_as(
    id: String,
    mime: String,
    default_name: String,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let src = state.assets.path_of(id.trim(), mime.trim())?;
    if !src.exists() {
        return Err(format!("资产文件不存在: {}", src.display()));
    }
    let mut builder = app.dialog().file();
    let name = default_name.trim();
    if !name.is_empty() {
        builder = builder.set_file_name(name);
    }
    let picked = builder.blocking_save_file();
    let Some(dest) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    let n = std::fs::copy(&src, &dest).map_err(|e| format!("保存失败: {e}"))?;
    tracing::info!(dest = %dest.display(), bytes = n, "asset saved as");
    Ok(Some(dest.display().to_string()))
}
