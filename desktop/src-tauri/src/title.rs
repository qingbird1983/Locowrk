//! 会话自动标题（v2.57，UI shell §8.2「对话行为偏好」+ §3.6）：首轮收束
//! 后旁车无标题时一次 LLM 调用生成 ≤25 字标题——替代首条消息截断的默认
//! 标题（ROADMAP §五 ㉙，PI session-title-summarize 借鉴）。开关
//! `chat.autoTitle` 缺省开（可配置原则：禁止项有开关）；**手动重命名过
//! （旁车有 title 字段）永不覆盖**；失败静默（标题是点缀，不值得报错）。
//!
//! 通路：agent_chat / 调度引擎轮次成功收束后 fire-and-forget（带出会话
//! 锁的消息快照，不阻塞返回值，⑬ extract 同款）；生成经 AppLlm::complete
//! （设置驱动，模型未配置直接跳过）；完成写旁车 + `lw://session-title`
//! 推送（前端更新侧栏与顶栏标题）。

use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use session_store::{read_meta, write_title, Message};
use tauri::{AppHandle, Emitter};

use crate::llm::AppLlm;
use crate::settings::SettingsStore;
use kernel_agent::LlmService;

/// 标题最大字符数（㉙：≤25 字）。
pub const TITLE_MAX_CHARS: usize = 25;

/// 题干里用户/助手各自截断宽度（控制一次性调用的上下文成本）。
const MATERIAL_CHARS: usize = 600;

/// `lw://session-title` 推送载荷（camelCase 对齐前端 types）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SessionTitlePayload {
    session_id: String,
    title: String,
}

/// 判定：该会话该不该生成标题（纯函数，测试锁定）——开关开 + 旁车无
/// 手动标题 + 有用户消息。
fn should_generate(auto_title: bool, sidecar_title: Option<&str>, messages: &[Message]) -> bool {
    auto_title
        && sidecar_title.is_none()
        && messages.iter().any(|m| matches!(m, Message::User { .. }))
}

/// 题干（纯函数）：首个用户消息 + 首个非空助手正文，各截 [`MATERIAL_CHARS`]。
fn title_material(messages: &[Message]) -> Option<String> {
    let user = messages.iter().find_map(|m| match m {
        Message::User { content } => Some(content.chars().take(MATERIAL_CHARS).collect::<String>()),
        _ => None,
    })?;
    let assistant = messages.iter().find_map(|m| match m {
        Message::Assistant { content, .. } if !content.is_empty() => {
            Some(content.chars().take(MATERIAL_CHARS).collect::<String>())
        }
        _ => None,
    });
    Some(match assistant {
        Some(a) => format!("用户：{user}\n\n助手：{a}"),
        None => format!("用户：{user}"),
    })
}

/// 标题净化（纯函数）：trim → 去成对包裹引号 → 取首行 → 截
/// [`TITLE_MAX_CHARS`]；空 = None（不写不推）。
fn sanitize_title(raw: &str) -> Option<String> {
    let t = raw
        .trim()
        .trim_matches(|c| matches!(c, '"' | '“' | '”' | '「' | '」' | '《' | '》'))
        .trim();
    let first_line = t.lines().next().unwrap_or("").trim();
    let clipped: String = first_line.chars().take(TITLE_MAX_CHARS).collect();
    (!clipped.is_empty()).then_some(clipped)
}

/// 轮次成功收束后的生成入口（fire-and-forget）。`messages` 是调用方带出
/// 会话锁的快照；模型未配置 / 开关关 / 旁车已有标题在这里就地跳过。
pub fn spawn_auto_title(
    app: AppHandle,
    settings: Arc<SettingsStore>,
    sessions_dir: PathBuf,
    session_id: String,
    messages: Vec<Message>,
) {
    tauri::async_runtime::spawn(async move {
        if let Err(e) = generate(&app, &settings, &sessions_dir, &session_id, &messages).await {
            tracing::warn!(session = %session_id, "auto title skipped: {e}");
        }
    });
}

async fn generate(
    app: &AppHandle,
    settings: &Arc<SettingsStore>,
    sessions_dir: &std::path::Path,
    session_id: &str,
    messages: &[Message],
) -> Result<(), String> {
    let chat = settings.chat();
    // 缺文件/坏 JSON = None（persist 契约：回落自动推导）
    let sidecar_title = read_meta(sessions_dir, session_id).and_then(|m| m.title);
    if !should_generate(chat.auto_title, sidecar_title.as_deref(), messages) {
        return Ok(());
    }
    let cfg = settings.llm();
    if cfg.model.trim().is_empty() {
        // 演示替身不生成标题（脚本回显当标题是噪声）
        return Ok(());
    }
    let material = title_material(messages).ok_or("没有用户消息")?;
    let req = kernel_agent::LlmRequest {
        messages: vec![Message::User {
            content: format!("请为下面这段对话生成一个不超过{TITLE_MAX_CHARS}个字的标题。只输出标题本身：不要引号、句号，不要任何前后缀说明。\n\n{material}"),
        }],
        ..Default::default()
    };
    let resp = AppLlm::new(Arc::clone(settings))
        .complete(req)
        .await
        .map_err(|e| e.to_string())?;
    let Some(title) = sanitize_title(&resp.content) else {
        return Err("生成结果为空".into());
    };
    write_title(sessions_dir, session_id, &title).map_err(|e| e.to_string())?;
    let payload = SessionTitlePayload {
        session_id: session_id.to_string(),
        title,
    };
    app.emit("lw://session-title", &payload)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests;
