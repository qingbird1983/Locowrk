//! 导入面板命令（批 F）：四源扫描（纯读预览）+ 选中项写入档位。
//! 密钥纪律：扫描面只回 `has_key`；apply 在后端重扫回配取 Key 落档。

use tauri::State;

use crate::llm::import::{self, ApplyReport, ScanOutcome};
use crate::state::AppState;

/// 用户主目录（四源都在 home 下；Windows USERPROFILE / Unix HOME）。
fn home_dir() -> Result<std::path::PathBuf, String> {
    std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map(std::path::PathBuf::from)
        .map_err(|_| "无法定位用户主目录".to_string())
}

/// 全源扫描（claude-code / codex / opencode / cc-switch，纯读不写）。
/// 单源失败记 `errors` 不拖垮其它源；候选不带 Key（`hasKey` 布尔）。
#[tauri::command]
pub async fn llm_import_scan() -> Result<ScanOutcome, String> {
    let home = home_dir()?;
    // 解析器是纯 CPU 读小文件，spawn_blocking 防 IPC 线程卡顿（cc-switch
    // 是 SQLite 打开）。
    tauri::async_runtime::spawn_blocking(move || import::scan(&home))
        .await
        .map_err(|e| format!("扫描任务失败: {e}"))
}

/// 导入选中项：新档追加落盘（label 带来源尾缀；同 kind+端点+模型已存 =
/// 跳过带因）。保存即热换（客户端逐请求新建），settings 惰性迁移把明文
/// Key 收进系统凭据库（批 C 通道）。
#[tauri::command]
pub async fn llm_import_apply(
    ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<ApplyReport, String> {
    let home = home_dir()?;
    let cloned = ids.clone();
    let settings = std::sync::Arc::clone(&state.settings);
    tauri::async_runtime::spawn_blocking(move || {
        let mut report = ApplyReport::default();
        settings
            .update_llm_config(|cfg| {
                report = import::apply(&home, &cloned, cfg);
                Ok(())
            })
            .map(|_| report)
    })
    .await
    .map_err(|e| format!("导入任务失败: {e}"))?
}
