//! 就地探测与诊断读数（都不落盘）：Ollama 上下文窗口、embedding 维度、
//! ⑫ 工具暴露的最近一次截断快照。前两件业务在 llm/probe.rs，第三件读 ToolExposure 的快照。

use tauri::State;

use kernel_agent::ExposureSnapshot;

use crate::state::AppState;

// ---- 模型窗口探测（S2，清单 §5）：Ollama /api/show 自动提取，读不到
// 由 UI 提示手动选择。业务在 llm/probe.rs。 ----

/// 探测 Ollama 模型的上下文窗口（token）。base_url 空 = 默认端点。
#[tauri::command]
pub async fn llm_probe_window(base_url: Option<String>, model: String) -> Result<u64, String> {
    if model.trim().is_empty() {
        return Err("模型名为空，无法探测".into());
    }
    crate::llm::probe::probe_context_window(base_url.as_deref(), model.trim()).await
}

/// 探测 embedding 端点（⑬）：按给定端点/模型嵌一条测试文本，返回向量
/// 维度（UI 显示"维度 768"之类，失败报 Err 由 UI 提示）。与保存解耦——
/// 这里只探测不落盘。
#[tauri::command]
pub async fn llm_probe_embedding(
    base_url: Option<String>,
    model: String,
    api_key: Option<String>,
) -> Result<usize, String> {
    if model.trim().is_empty() {
        return Err("embedding 模型名为空，无法探测".into());
    }
    let client = crate::llm::EmbedClient {
        base_url: base_url
            .unwrap_or_else(|| "http://localhost:11434".into())
            .trim_end_matches('/')
            .to_string(),
        model: model.trim().to_string(),
        api_key: api_key.filter(|k| !k.is_empty()),
    };
    client
        .embed(&["locowrk probe".to_string()])
        .await
        .and_then(|vecs| {
            vecs.first()
                .map(|v| v.len())
                .ok_or_else(|| "embedding 响应为空".into())
        })
}

// ---- 工具暴露调试命令（⑫，exposure 文档 §6.3/§9.6）：开发者选项读最近
// 一次截断快照。快照由 ToolExposure 在每个 agent/request 上刷新，前端只在
// 打开面板时拉一次（§8.8"用户来找"的信息，不推送不轮询）。 ----

/// 最近一次请求的工具暴露快照（总数/暴露数/always/会话热/全局热/节省估算）。
#[tauri::command]
pub async fn tool_exposure_debug(state: State<'_, AppState>) -> Result<ExposureSnapshot, String> {
    Ok(state.exposure.snapshot())
}
