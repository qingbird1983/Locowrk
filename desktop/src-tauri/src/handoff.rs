//! handoff 收束自动更新（F3，《Memory File-ization Design》§4 已裁定③）：
//! 归属会话轮次成功收束后 fire-and-forget 一次 LLM 调用——读当前
//! `<repo>/.Locowrk/handoff.md`（可缺）+ 本轮转录，产出全量新内容落回
//! 同一文件。写边界宁严勿松（设计 §1.6）= 只写 handoff.md 这一个文件；
//! 未归属会话跳过；模型未配置跳过（演示替身同 title 纪律）；失败只
//! warn——交接是背景维护，不值得打扰聊天。首次创建推 `lw://handoff-created`
//! （UI shell §4.1：前端 toast 如实告知一次，已有文件的更新不发生）。
//!
//! 缓存纪律（设计 §3「mtime 触发注入缓存键更新」）：内容未变不落盘——
//! mtime 不动 → scope_id 指纹不换 → M1 前缀缓存不失效；实质进展轮的
//! 一次计划内失效属设计内代价，提示词同时要求"无变化回 UNCHANGED"双保险。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use session_store::Message;
use tauri::{AppHandle, Emitter};

use crate::llm::AppLlm;
use crate::settings::SettingsStore;
use kernel_agent::LlmService;

/// 产出内容上限（字符）：提示词要求 ≤40 行，此为防失控的硬顶。
const CONTENT_MAX_CHARS: usize = 8000;

/// `lw://handoff-created` 推送载荷（camelCase 对齐前端 types）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct HandoffCreatedPayload {
    repo: String,
    path: String,
}

/// handoff 更新提示词（LLM 面，英文与提取/标题同语言）：全量改写 +
/// 保留仍有效的用户内容 + 无变化回 UNCHANGED（不落盘不换缓存键）。
const HANDOFF_SYSTEM: &str = "You maintain the project's handoff file (.Locowrk/handoff.md) \
so a future session can resume work without re-reading history. You are given the current \
file content (may be empty) and an excerpt of the conversation turn that just finished. \
Rewrite the FULL new content in Markdown with sections: `## Current state` (what is done / \
in progress), `## Next steps` (concrete and actionable), `## Notes` (key decisions, \
pitfalls, open questions). Keep the whole file under 40 lines. Preserve still-valid \
user-authored content; drop only what is outdated. If this turn does not change the \
project state, next steps, or notes, reply with exactly UNCHANGED. Reply with ONLY the \
file content (no code fences, no commentary), or UNCHANGED.";

/// 提示词材料（纯函数）：当前 handoff 全文（可缺）+ 本轮转录。
fn material(current: &str, transcript: &str) -> String {
    let current = if current.trim().is_empty() {
        "(empty — first creation)".to_string()
    } else {
        current.to_string()
    };
    format!("--- current handoff.md ---\n{current}\n\n--- finished turn ---\n{transcript}")
}

/// 产出净化（纯函数）：trim → `UNCHANGED` / 空 = None（不写）→ 剥一层
/// 代码围栏（模型常把整文件包进 ```markdown）→ 截 [`CONTENT_MAX_CHARS`]。
fn sanitize_content(raw: &str) -> Option<String> {
    let t = raw.trim();
    if t.is_empty() || t.eq_ignore_ascii_case("UNCHANGED") {
        return None;
    }
    let body = if t.starts_with("```") {
        let after = t.split_once('\n').map(|x| x.1).unwrap_or("");
        match after.rfind("```") {
            Some(end) => after[..end].trim(),
            None => after.trim(),
        }
    } else {
        t
    };
    let body = body.trim();
    if body.is_empty() {
        return None;
    }
    let clipped: String = body.chars().take(CONTENT_MAX_CHARS).collect();
    Some(clipped)
}

/// 轮次成功收束后的更新入口（fire-and-forget，agent_chat / 调度引擎两收
/// 束点同挂）。`messages` 是调用方带出会话锁的快照；开关关 / 未归属 /
/// 模型未配置在这里就地跳过。
pub fn spawn_handoff_update(
    app: AppHandle,
    settings: Arc<SettingsStore>,
    sessions_dir: PathBuf,
    session_id: String,
    messages: Vec<Message>,
) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = update(&app, &settings, &sessions_dir, &session_id, &messages).await {
            tracing::warn!(session = %session_id, "handoff update skipped: {e}");
        }
    });
}

async fn update(
    app: &AppHandle,
    settings: &Arc<SettingsStore>,
    sessions_dir: &Path,
    session_id: &str,
    messages: &[Message],
) -> Result<(), String> {
    if !settings.memory().handoff_update {
        return Ok(());
    }
    let repo =
        crate::workspace::repo_root_of(sessions_dir, session_id).ok_or("未归属或目录缺失")?;
    if settings.llm().model.trim().is_empty() {
        // 演示替身不维护交接（脚本回显当交接是噪声，同 title 纪律）
        return Ok(());
    }
    // 本轮切片（最后一次用户消息起到末尾，extract 同口径）；无用户消息
    // 的轮次（纯系统注入）没有"发生了什么"可总结，跳过。
    let turn = crate::extract::turn_messages(messages);
    if !turn.iter().any(|m| matches!(m, Message::User { .. })) {
        return Err("turn has no user message".into());
    }
    let transcript = crate::extract::build_transcript(&turn);
    let handoff_path = repo
        .join(crate::repo_context::DIR)
        .join(crate::repo_context::HANDOFF);
    let current = std::fs::read_to_string(&handoff_path).unwrap_or_default();
    let req = kernel_agent::LlmRequest {
        messages: vec![Message::user(material(&current, &transcript))],
        system: Some(HANDOFF_SYSTEM.into()),
        // 辅助调用标记（批 B §5.4）：推理档降到方言最低档、不发 max_tokens
        params: serde_json::json!({ "aux": true }),
        ..Default::default()
    };
    let resp = AppLlm::new(Arc::clone(settings))
        .complete(req)
        .await
        .map_err(|e| e.to_string())?;
    let Some(content) = sanitize_content(&resp.content) else {
        // UNCHANGED / 空 = 无新闻：不落盘（mtime 不动 → 缓存键不换）
        return Ok(());
    };
    if content.trim_end() == current.trim_end() {
        return Ok(());
    }
    crate::repo_context::ensure_layout(&repo)?;
    let existed = handoff_path.exists();
    plugin_api::memlayout::atomic_write(&handoff_path, &content)?;
    if !existed {
        let payload = HandoffCreatedPayload {
            repo: repo.to_string_lossy().into_owned(),
            path: handoff_path.to_string_lossy().into_owned(),
        };
        app.emit("lw://handoff-created", &payload)
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
