//! 回合后异步提取（⑬，Context 文档 §5.1 ⑤ / §4.2.2"隐式提取放最后，
//! 易记垃圾"）：轮次收敛后 spawn 一个 fire-and-forget 任务——LLM 从本轮
//! 对话提取 ①"值得记住的事实"（写 memory-sqlite，source=extracted）+
//! ②一段经历摘要（embedding 后写 experiences.sqlite3，语义召回的数据
//! 源）。settings `memory.autoExtract` 门控（缺省关），任何失败只 warn，
//! 绝不影响聊天；事实 key 强制 `auto.` 前缀（永不覆盖用户显式记忆），
//! 每轮至多 3 条防垃圾灌库。

use std::sync::Arc;

use kernel_agent::{EmbedPort, LlmRequest, LlmService};
use serde_json::Value;
use session_store::Message;

use crate::experiences::ExperienceDb;
use crate::llm::CooledEmbed;
use crate::memory::MemoryStore;

/// 事实条数上限（§4.2.2"易记垃圾"的频控第一道闸）。
const MAX_FACTS: usize = 3;
/// 单条 value 上限（字符）。
const FACT_VALUE_MAX_CHARS: usize = 2000;
/// 经历摘要下限：太短的一句话不值得入库检索。
const EPISODE_MIN_CHARS: usize = 10;
/// 经历摘要上限（字符，注入侧 500 同口径）。
const EPISODE_MAX_CHARS: usize = 500;
/// 转录总量上限（字符）——提取输入不能比压缩输入还贵。
const TRANSCRIPT_MAX_CHARS: usize = 6000;

/// 提取所需依赖（组装点一次备齐；全部 Arc，spawn 走 'static）。
#[derive(Clone)]
pub struct ExtractDeps {
    pub llm: Arc<dyn LlmService>,
    pub memory: Arc<MemoryStore>,
    /// 经历库（embedding 未配置时组装点给 None——只提事实）。
    pub experiences: Option<Arc<ExperienceDb>>,
    pub embed: Arc<CooledEmbed>,
    /// 经历记忆开关快照（M5 `memory.experiences` 缺省关，树 v2 §10.3）：
    /// false = 本轮跳过经历提取（事实提取不受影响；逐轮快照 = 热换）。
    pub experiences_enabled: bool,
}

/// 提取提示词（LLM 面，英文与工具描述同语言）：只要 JSON、宁缺勿滥。
const EXTRACT_SYSTEM: &str = "You are a memory extraction assistant. Read the conversation \
excerpt and respond with ONLY a JSON object (no markdown, no commentary):\n\
{\"facts\": [{\"key\": \"short.keystyle.name\", \"value\": \"...\", \"tags\": \"comma,separated\"}], \"episode\": \"...\"}\n\
Rules:\n\
- facts: at most 3 durable facts worth remembering across sessions (user preferences, \
project constraints, decisions made). Skip small talk, transient details and anything \
already obvious. Empty array if nothing is worth remembering.\n\
- episode: one sentence (max 200 chars) summarizing what was discussed and decided; \
use null if the exchange was trivial.\n\
- Never invent facts. Keys are short ASCII identifiers like user.language or project.db_choice.";

/// 轮次转录（给 LLM 的提取输入）：User / Assistant 正文 + 工具调用痕迹。
/// F3 handoff 收束更新复用同一转录口径。
pub(crate) fn build_transcript(turn: &[Message]) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut budget = TRANSCRIPT_MAX_CHARS;
    for m in turn {
        if budget == 0 {
            break;
        }
        let line = match m {
            Message::User { content } => Some(format!("User: {}", clip(content, 1000))),
            Message::Assistant { content, .. } if !content.trim().is_empty() => {
                Some(format!("Assistant: {}", clip(content, 1000)))
            }
            Message::ToolResult { tool, error, .. } => Some(format!(
                "Tool `{tool}` {}",
                if error.is_some() {
                    "failed"
                } else {
                    "succeeded"
                }
            )),
            _ => None,
        };
        if let Some(l) = line {
            budget = budget.saturating_sub(l.chars().count());
            lines.push(l);
        }
    }
    lines.join("\n")
}

fn clip(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// 解析产出：facts（key/value/tags）+ 可选经历摘要。
type ParsedReply = (Vec<(String, String, String)>, Option<String>);

/// 解析 LLM 回复：剥代码围栏、取首个 `{` 到末个 `}`，产出 (facts, episode)。
fn parse_reply(text: &str) -> Option<ParsedReply> {
    let trimmed = text.trim().trim_matches('`');
    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    let v: Value = serde_json::from_str(&trimmed[start..=end]).ok()?;
    let mut facts = Vec::new();
    if let Some(arr) = v.get("facts").and_then(Value::as_array) {
        for f in arr {
            let key = f.get("key").and_then(Value::as_str).unwrap_or_default();
            let value = f.get("value").and_then(Value::as_str).unwrap_or_default();
            let tags = f.get("tags").and_then(Value::as_str).unwrap_or_default();
            if !key.trim().is_empty() && !value.trim().is_empty() {
                facts.push((
                    key.trim().to_string(),
                    value.trim().to_string(),
                    tags.trim().to_string(),
                ));
            }
        }
    }
    let episode = v
        .get("episode")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Some((facts, episode))
}

/// 事实 key 清洗：小写、非法字符转 `_`、强制 `auto.` 命名空间（永不
/// 覆盖用户显式记忆）。
pub(crate) fn sanitize_fact_key(key: &str) -> Option<String> {
    let cleaned: String = key
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let cleaned = cleaned.trim_matches(|c: char| c == '.' || c == '_');
    if cleaned.is_empty() {
        return None;
    }
    Some(if cleaned.starts_with("auto.") {
        cleaned.to_string()
    } else {
        format!("auto.{cleaned}")
    })
}

/// 提取一轮（可测主体）：转录 → LLM → 落库。全部失败路径返回 Err 文案。
pub async fn run_turn_extract(
    deps: &ExtractDeps,
    session_id: &str,
    turn: &[Message],
) -> Result<(usize, bool), String> {
    let transcript = build_transcript(turn);
    if !turn.iter().any(|m| matches!(m, Message::User { .. })) {
        return Err("turn has no user message".into());
    }
    let reply = deps
        .llm
        .complete(LlmRequest {
            messages: vec![Message::User {
                content: transcript,
            }],
            tools: Vec::new(),
            model: None,
            params: serde_json::json!({}),
            system: Some(EXTRACT_SYSTEM.into()),
            session_id: None,
        })
        .await
        .map_err(|e| format!("extract llm call failed: {e}"))?;
    let Some((facts, episode)) = parse_reply(&reply.content) else {
        return Err("extract reply is not parseable JSON".into());
    };
    // ㉘：键位 = 该会话归属的项目根（workspace.rs 单一真源；未归属 = ""）
    let workspace = deps.memory.scope_of_session(Some(session_id));
    // ① 事实 → memory（频控 + 命名空间隔离 + 项目键位）
    let mut written = 0usize;
    for (key, value, tags) in facts.into_iter().take(MAX_FACTS) {
        let Some(key) = sanitize_fact_key(&key) else {
            continue;
        };
        let value = clip(&value, FACT_VALUE_MAX_CHARS);
        match deps
            .memory
            .write(&workspace, &key, &value, &tags, "extracted")
        {
            Ok(()) => written += 1,
            Err(e) => tracing::warn!("extract memory write failed: {e}"),
        }
    }
    // ② 经历 → embedding → experiences（开关关/embedding 未配置/失败 =
    // 跳过；M5 §10.3 经历向量层缺省关）
    let mut episode_saved = false;
    if deps.experiences_enabled {
        if let Some(db) = &deps.experiences {
            if let Some(episode) = episode {
                let char_count = episode.chars().count();
                if char_count >= EPISODE_MIN_CHARS {
                    let episode = clip(&episode, EPISODE_MAX_CHARS);
                    match deps.embed.embed(std::slice::from_ref(&episode)).await {
                        Ok(vecs) if vecs.len() == 1 && !vecs[0].is_empty() => {
                            match db.insert(&episode, &vecs[0], session_id, &workspace) {
                                Ok(()) => episode_saved = true,
                                Err(e) => tracing::warn!("episode insert failed: {e}"),
                            }
                        }
                        Ok(_) | Err(_) => {
                            tracing::warn!("episode embedding unavailable; episode skipped")
                        }
                    }
                }
            }
        }
    }
    Ok((written, episode_saved))
}

/// 本轮消息切片：最后一次用户消息起到末尾（run_turn 刚追加的用户消息
/// 即本轮起点；历史轮次不参与提取）。克隆快照，spawn 走 'static。
pub(crate) fn turn_messages(all: &[Message]) -> Vec<Message> {
    let start = all
        .iter()
        .rposition(|m| matches!(m, Message::User { .. }))
        .unwrap_or(0);
    all[start..].to_vec()
}

/// 组装点入口：轮次收敛后调用（settings 门控在调用侧完成）。fire-and-
/// forget——提取失败不影响聊天，也不给 UI 发事件（结果在记忆管理页可见，
/// 来源徽章"自动提取"）。
pub fn spawn_turn_extract(deps: ExtractDeps, session_id: String, turn: Vec<Message>) {
    tokio::spawn(async move {
        match run_turn_extract(&deps, &session_id, &turn).await {
            Ok((facts, episode)) => {
                tracing::info!(session = %session_id, facts, episode, "turn extract done")
            }
            Err(e) => tracing::warn!(session = %session_id, "turn extract skipped: {e}"),
        }
    });
}

#[cfg(test)]
mod tests;
