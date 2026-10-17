//! 检索信号（⑬）（自 `tests.rs` 批 F-16 拆出，用例本体逐字未改）：无热度时相似度
//! 命中者补位并归因 retrieved / 无检索器保持两层打分旧行为。

use super::*;

use session_store::Message;

/// 检索信号（⑬）：无会话/全局热度时，相似度命中者靠 0.5 × sim 入选，
/// 快照归因 retrieved（假嵌入器按 "rust" 关键词给一维向量）。
#[tokio::test]
async fn retrieval_signal_fills_quota_and_attributes() {
    use crate::middleware::tool_retrieval::EmbedPort;

    struct Kw;
    #[async_trait::async_trait]
    impl EmbedPort for Kw {
        async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            Ok(texts
                .iter()
                .map(|t| {
                    if t.contains("rust") {
                        vec![1.0]
                    } else {
                        vec![0.0]
                    }
                })
                .collect())
        }
    }

    let tools = vec![
        spec("rust_commit", Exposure::OnDemand), // 描述含 "rust" → 相似度 1
        spec("http_post", Exposure::OnDemand),
        spec("fs_read", Exposure::OnDemand),
    ];
    let messages = vec![Message::User {
        content: "please work on the rust project".into(),
        parts: Vec::new(),
    }];
    let l = ToolExposure::new(
        Arc::new(Fixed(HashMap::new())),
        cfg_lock(true, 1),
        Some(Arc::new(Retriever::new(Arc::new(Kw)))),
    );
    let (decision, out) = run(&l, tools, messages).await;
    assert!(matches!(decision, WaterfallDecision::Continue));
    assert_eq!(out.tools.len(), 1);
    assert_eq!(out.tools[0].name, "rust_commit");
    let snap = l.snapshot();
    assert_eq!(snap.retrieved, vec!["rust_commit"]);
    assert!(snap.session_hot.is_empty() && snap.global_hot.is_empty());
}

/// 无检索器（embedding 未配置）= 既有两层打分行为不变。
#[tokio::test]
async fn no_retriever_keeps_legacy_scoring() {
    let tools = vec![spec("a", Exposure::OnDemand), spec("b", Exposure::OnDemand)];
    let l = listener(HashMap::new(), cfg_lock(true, 1));
    let (decision, out) = run(&l, tools, Vec::new()).await;
    assert!(matches!(decision, WaterfallDecision::Continue));
    assert_eq!(out.tools.len(), 1);
    assert!(l.snapshot().retrieved.is_empty());
}
