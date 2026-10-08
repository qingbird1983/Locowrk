//! extract 单测：回复解析 / key 清洗命名空间 / 转录裁剪 / 提取主流程
//! （替身 LlmService + 真实 memory 库）。

use super::*;
use crate::llm::SharedEmbed;
use kernel_agent::LlmResponse;
use std::sync::Mutex;

fn msg_user(text: &str) -> Message {
    Message::user(text)
}

// ---- parse_reply ----

#[test]
fn parse_reply_plain_fenced_and_noisy() {
    let (facts, episode) = parse_reply(
        r#"{"facts":[{"key":"user.lang","value":"zh-CN","tags":"pref"}],"episode":"chose sqlite"}"#,
    )
    .unwrap();
    assert_eq!(facts.len(), 1);
    assert_eq!(facts[0].0, "user.lang");
    assert_eq!(episode.as_deref(), Some("chose sqlite"));

    // 代码围栏 + 前后噪声
    let (_, episode) =
        parse_reply("```json\n{\"facts\":[],\"episode\":\"did rust stuff\"}\n```").unwrap();
    assert_eq!(episode.as_deref(), Some("did rust stuff"));

    // 空 facts / null episode / 空值过滤
    let (facts, episode) = parse_reply(
        r#"{"facts":[{"key":"","value":"v"},{"key":"k","value":"  "}],"episode":null}"#,
    )
    .unwrap();
    assert!(facts.is_empty());
    assert!(episode.is_none());
    // 非 JSON = None
    assert!(parse_reply("I refuse to comply").is_none());
}

// ---- sanitize_fact_key ----

#[test]
fn sanitize_fact_key_forces_auto_namespace() {
    assert_eq!(
        sanitize_fact_key("User.Language").as_deref(),
        Some("auto.user.language")
    );
    assert_eq!(sanitize_fact_key("auto.db").as_deref(), Some("auto.db"));
    // 纯中文 key 清洗后全为下划线 → trim 空 → None（key 约定 ASCII 标识符）
    assert_eq!(sanitize_fact_key("数据 库!"), None);
    assert_eq!(sanitize_fact_key("..."), None);
    assert_eq!(sanitize_fact_key("  "), None);
}

// ---- build_transcript ----

#[test]
fn transcript_renders_turn_shapes() {
    let turn = vec![
        msg_user("remember my pref"),
        Message::Assistant {
            content: String::new(),
            tool_calls: vec![],
        },
        Message::ToolResult {
            call_id: plugin_api::CallId("c1".into()),
            tool: "memory_write".into(),
            args: serde_json::json!({}),
            output: serde_json::json!({}),
            error: None,
        },
        Message::Assistant {
            content: "done".into(),
            tool_calls: vec![],
        },
    ];
    let t = build_transcript(&turn);
    assert!(t.contains("User: remember my pref"));
    assert!(t.contains("Tool `memory_write` succeeded"));
    assert!(t.contains("Assistant: done"));
    // 空正文 assistant 不出空行条目
    assert_eq!(t.matches("Assistant:").count(), 1);
}

// ---- run_turn_extract 主流程 ----

/// 替身 LLM：返回预设文本，记录收到的请求。
struct FakeLlm(Mutex<String>);

#[async_trait::async_trait]
impl LlmService for FakeLlm {
    async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, kernel_agent::LlmError> {
        Ok(LlmResponse {
            content: self.0.lock().unwrap().clone(),
            tool_calls: Vec::new(),
            usage: None,
        })
    }
}

fn deps_with(llm: FakeLlm) -> (ExtractDeps, Arc<ExperienceDb>, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "locowrk-extract-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    // 测试解析器：所有会话未归属（键位 ""）
    let memory = Arc::new(MemoryStore::new(&dir, Arc::new(|_: &str| None::<String>)));
    let db = Arc::new(ExperienceDb::open(&dir.join("experiences.sqlite3")).unwrap());
    // 未配置 embedding 的共享柄 = embed 恒 Err（经历路径降级）
    let embed = SharedEmbed::new();
    (
        ExtractDeps {
            llm: Arc::new(llm),
            memory,
            experiences: Some(db.clone()),
            embed: embed.port(),
            experiences_enabled: true, // 本夹具专测 embed 故障降级，开关置开
        },
        db,
        dir,
    )
}

#[tokio::test]
async fn extracts_facts_but_skips_episode_when_embed_down() {
    let reply = r#"{"facts":[
        {"key":"User.Language","value":"zh-CN","tags":"pref"},
        {"key":"already.namespaced","value":"x"},
        {"key":"","value":"skip"},
        {"key":"four","value":"this one is capped out"}
    ],"episode":"decided to keep the memory contract simple"}"#;
    let (deps, db, dir) = deps_with(FakeLlm(Mutex::new(reply.into())));
    let turn = vec![msg_user("记住我用中文")];
    let (written, episode_saved) = run_turn_extract(&deps, "s1", &turn).await.unwrap();
    // parse 层已滤空 key/value → 剩 3 条恰在 MAX_FACTS 上限内全写，
    // 第 4 条被 take(3) 截掉
    assert_eq!(written, 3);
    // embedding 不可用 → 经历跳过但事实照写
    assert!(!episode_saved);
    assert_eq!(db.count().unwrap(), 0);
    let rows = deps.memory.list("", 50).unwrap();
    assert!(rows
        .iter()
        .any(|r| r.key == "auto.user.language" && r.source == "extracted"));
    assert!(rows.iter().all(|r| r.key.starts_with("auto.")));
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn no_user_message_is_error() {
    let (deps, _db, dir) = deps_with(FakeLlm(Mutex::new("{}".into())));
    assert!(run_turn_extract(&deps, "s1", &[]).await.is_err());
    std::fs::remove_dir_all(&dir).ok();
}
