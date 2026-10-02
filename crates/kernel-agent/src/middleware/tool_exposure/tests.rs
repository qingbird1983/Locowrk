//! ToolExposure 单测：always 直通 / Top-N 截断 / 会话内优先 / 全局 EWMA /
//! 停用直通 / 快照归因 / fail-open。

use super::*;
use cordis_core::WaterfallListener;
use plugin_api::SideEffect;
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
    Arc::new(RwLock::new(ToolExposureConfig { enabled, max_tools }))
}

fn listener(ewma: HashMap<String, f64>, cfg: Arc<RwLock<ToolExposureConfig>>) -> ToolExposure {
    ToolExposure::new(Arc::new(Fixed(ewma)), cfg, None)
}

/// 带一条 `tool` 的 ToolResult（该工具在本会话内用过）。
fn messages_with_session_use(tool: &str) -> Vec<Message> {
    vec![
        Message::User {
            content: "do it".into(),
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
    let payload = serde_json::to_value(RequestPayload {
        messages,
        model: None,
        params: json!({}),
        tools,
        compacted: None,
        system: None,
        session_id: None,
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

#[tokio::test]
async fn always_pass_through_and_top_n_fills_quota() {
    let tools = vec![
        spec("fs_read", Exposure::Always),
        spec("search_tools", Exposure::Always),
        spec("a1", Exposure::OnDemand),
        spec("a2", Exposure::OnDemand),
        spec("a3", Exposure::OnDemand),
    ];
    let l = listener(HashMap::new(), cfg_lock(true, 3));
    let (decision, out) = run(&l, tools, Vec::new()).await;
    assert!(matches!(decision, WaterfallDecision::Replace(_)));
    // always 2 个直通不占额度，quota = 3 - 2 = 1，只补一个候选
    let names: Vec<&str> = out.tools.iter().map(|t| t.name.as_str()).collect();
    assert_eq!(names, vec!["fs_read", "search_tools", "a1"]);
    let snap = l.snapshot();
    assert_eq!(snap.total, 5);
    assert_eq!(snap.exposed, 3);
    assert_eq!(snap.always, vec!["fs_read", "search_tools"]);
    assert!(snap.saved_tokens_estimate > 0);
}

#[tokio::test]
async fn scoring_orders_session_and_global() {
    let tools = vec![
        spec("git_diff", Exposure::OnDemand),
        spec("cold", Exposure::OnDemand),
    ];
    // cold 全局很热（0.7×5=3.5 > 会话 1.0）：总分排序 cold 赢
    let ewma: HashMap<String, f64> = [("cold".into(), 5.0)].into();
    let l = listener(ewma, cfg_lock(true, 1));
    let (_, out) = run(&l, tools.clone(), messages_with_session_use("git_diff")).await;
    assert_eq!(out.tools[0].name, "cold");
    let snap = l.snapshot();
    assert_eq!(snap.global_hot, vec!["cold"]);
    assert!(snap.session_hot.is_empty());

    // cold 无全局分时，会话内用过的 git_diff 稳进
    let l2 = listener(HashMap::new(), cfg_lock(true, 1));
    let (_, out2) = run(&l2, tools, messages_with_session_use("git_diff")).await;
    assert_eq!(out2.tools[0].name, "git_diff");
    assert_eq!(l2.snapshot().session_hot, vec!["git_diff"]);
}

#[tokio::test]
async fn session_hot_is_boolean_not_count() {
    // 会话内用过 2 次与 1 次同权重（1.0），不叠加
    let mut messages = messages_with_session_use("t1");
    messages.extend(messages_with_session_use("t1"));
    let tools = vec![
        spec("t1", Exposure::OnDemand),
        spec("t2", Exposure::OnDemand),
    ];
    let l = listener(HashMap::new(), cfg_lock(true, 2));
    let (_, out) = run(&l, tools, messages).await;
    assert_eq!(out.tools.len(), 2);
    assert_eq!(l.snapshot().session_hot, vec!["t1"]);
}

#[tokio::test]
async fn disabled_or_zero_max_is_passthrough() {
    let tools = vec![spec("a", Exposure::OnDemand), spec("b", Exposure::OnDemand)];
    let l = listener(HashMap::new(), cfg_lock(false, 1));
    let (decision, out) = run(&l, tools.clone(), Vec::new()).await;
    assert!(matches!(decision, WaterfallDecision::Continue));
    assert_eq!(out.tools.len(), 2);
    assert!(!l.snapshot().enabled);

    let l2 = listener(HashMap::new(), cfg_lock(true, 0));
    let (d2, out2) = run(&l2, tools, Vec::new()).await;
    assert!(matches!(d2, WaterfallDecision::Continue));
    assert_eq!(out2.tools.len(), 2);
}

#[tokio::test]
async fn no_trimming_needed_still_records_snapshot() {
    let tools = vec![spec("a", Exposure::OnDemand)];
    let l = listener(HashMap::new(), cfg_lock(true, 20));
    let (_, out) = run(&l, tools, Vec::new()).await;
    assert_eq!(out.tools.len(), 1);
    let snap = l.snapshot();
    assert!(snap.enabled);
    assert_eq!(snap.total, 1);
    assert_eq!(snap.exposed, 1);
    assert_eq!(snap.saved_tokens_estimate, 0);
    // 无会话无热度：纯补位不归组
    assert!(snap.session_hot.is_empty() && snap.global_hot.is_empty());
}

#[tokio::test]
async fn non_request_key_is_noop() {
    let l = listener(HashMap::new(), cfg_lock(true, 1));
    let payload = serde_json::to_value(RequestPayload {
        messages: Vec::new(),
        model: None,
        params: json!({}),
        tools: vec![spec("a", Exposure::OnDemand)],
        compacted: None,
        system: None,
        session_id: None,
    })
    .expect("payload");
    let d = l.handle(points::PRE_STEP, &payload).await;
    assert!(matches!(d, WaterfallDecision::Continue));
}

#[tokio::test]
async fn malformed_payload_fails_open() {
    let l = listener(HashMap::new(), cfg_lock(true, 1));
    let d = l
        .handle(points::REQUEST, &json!({"messages": "garbage"}))
        .await;
    assert!(matches!(d, WaterfallDecision::Continue));
}

#[test]
fn session_counts_ignores_empty_tool_names() {
    let messages = vec![Message::ToolResult {
        call_id: plugin_api::CallId("x".into()),
        tool: String::new(),
        args: json!({}),
        output: json!(null),
        error: None,
    }];
    assert!(session_counts(&messages).is_empty());
}

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
    }];
    let l = ToolExposure::new(
        Arc::new(Fixed(HashMap::new())),
        cfg_lock(true, 1),
        Some(Arc::new(Retriever::new(Arc::new(Kw)))),
    );
    let (decision, out) = run(&l, tools, messages).await;
    assert!(matches!(decision, WaterfallDecision::Replace(_)));
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
    assert!(matches!(decision, WaterfallDecision::Replace(_)));
    assert_eq!(out.tools.len(), 1);
    assert!(l.snapshot().retrieved.is_empty());
}
