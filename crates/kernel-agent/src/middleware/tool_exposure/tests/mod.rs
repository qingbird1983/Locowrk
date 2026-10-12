//! ToolExposure 单测：always 直通 / Top-N 截断 / 会话内优先 / 全局 EWMA /
//! 停用直通 / 快照归因 / fail-open。
//! 批 F-16 目录化：共享假件与请求跑批入口留本文件，用例按面分簇见 `scoring` /
//! `passthrough` / `retrieval` / `pin_session`。

mod passthrough;
mod pin_session;
mod retrieval;
mod scoring;

use super::*;
use cordis_core::WaterfallListener;
// 批 F-23 起这两个名字**不再**经父 glob 传导：`ToolSpec` 与 `RETRIEVAL_WEIGHT` 随实现
// 搬进 `tool_exposure::{measure, selection}`，父文件的 import 面收窄成只剩 `Exposure`
// 与 `Retriever`（clippy `--all-targets` 把留在父里的判成 unused_imports）。测试面
// 自己引用 ⇒ 按 F-14「`use` 按引用面分派」在这里显式引入，子簇再经 `use super::*` 拿。
use crate::middleware::tool_retrieval::RETRIEVAL_WEIGHT;
use plugin_api::{SideEffect, ToolSpec};
use serde_json::json;
use session_store::Message;

/// 假全局热度表。
struct Fixed(HashMap<String, f64>);

impl GlobalEwma for Fixed {
    fn ewma_scores(&self) -> HashMap<String, f64> {
        self.0.clone()
    }
}

fn spec(name: &str, exposure: Exposure) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: format!("{name} description"),
        schema: json!({"type": "object"}),
        side_effect: SideEffect::Pure,
        exposure,
    }
}

fn cfg_lock(enabled: bool, max_tools: usize) -> Arc<RwLock<ToolExposureConfig>> {
    Arc::new(RwLock::new(ToolExposureConfig {
        enabled,
        max_tools,
        ..ToolExposureConfig::default()
    }))
}

fn listener(ewma: HashMap<String, f64>, cfg: Arc<RwLock<ToolExposureConfig>>) -> ToolExposure {
    ToolExposure::new(Arc::new(Fixed(ewma)), cfg, None)
}

/// 带一条 `tool` 的 ToolResult（该工具在本会话内用过）。
fn messages_with_session_use(tool: &str) -> Vec<Message> {
    vec![
        Message::User {
            content: "do it".into(),
            parts: Vec::new(),
        },
        Message::ToolResult {
            call_id: plugin_api::CallId("c1".into()),
            tool: tool.into(),
            args: json!({}),
            output: json!(null),
            error: None,
        },
    ]
}

async fn run(
    listener: &ToolExposure,
    tools: Vec<ToolSpec>,
    messages: Vec<Message>,
) -> (WaterfallDecision, RequestPayload) {
    run_session(listener, tools, messages, None).await
}

/// 带会话身份的请求（M1 钉死只作用于带身份的会话）。
async fn run_session(
    listener: &ToolExposure,
    tools: Vec<ToolSpec>,
    messages: Vec<Message>,
    session_id: Option<&str>,
) -> (WaterfallDecision, RequestPayload) {
    let payload = serde_json::to_value(RequestPayload {
        messages,
        model: None,
        params: json!({}),
        tools,
        compacted: None,
        system: None,
        session_id: session_id.map(str::to_string),
        tree_directory: None,
    })
    .expect("payload");
    let decision = listener.handle(points::REQUEST, &payload).await;
    let source = match &decision {
        WaterfallDecision::Replace(v) => v.clone(),
        _ => payload,
    };
    let out = serde_json::from_value::<RequestPayload>(source).expect("payload parses");
    (decision, out)
}
