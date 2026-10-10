//! 浏览器面板命令（UI shell 文档 §2.5 浏览器页签，v2.144）：前端把预览
//! 页桥接脚本回传的 console 批次推入账本（模型 `browser_console` 工具
//! 读出）。薄适配层：只转参数，缓冲纪律在 `browser::ConsoleStore`。

use tauri::State;

use crate::browser::ConsoleReport;
use crate::state::AppState;

/// 前端 console 回传（每页签环形钳 200 条由账本承担；空批次无害）。
#[tauri::command]
pub async fn browser_report_errors(
    tab_id: String,
    report: ConsoleReport,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.browser.report(&tab_id, report.entries);
    Ok(())
}
