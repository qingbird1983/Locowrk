//! 会话导出（v2.49 会话运营，UI shell 文档 §3.6）：Markdown 转录渲染。
//! 数据 = 会话日志的同一投影（`runner::project_events`），与转录 / 历史
//! 重放同源；格式按人类阅读组织——用户 / 助手标题段 + 工具行 / 审批记录
//! 引用行。工具参数与长输出截断并标注；无损归档用 jsonl 原件本身，导出
//! 不追求无损。

use crate::events::AgentEvent;

/// 工具参数 / 结果单条截断宽度（字符）。
const CLIP: usize = 400;

fn clip(s: &str) -> String {
    let mut out: String = s.chars().take(CLIP).collect();
    if s.chars().count() > CLIP {
        out.push_str("…（截断）");
    }
    out.replace('\n', " ")
}

fn clip_json(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "(无参数)".into(),
        other => clip(&other.to_string()),
    }
}

fn section(out: &mut String, header: &str, body: &str) {
    if body.trim().is_empty() {
        return;
    }
    out.push_str(header);
    out.push_str(body.trim());
    out.push_str("\n\n");
}

/// 渲染 Markdown 转录。连续 `chunk` 聚合为一个「助手」段（与转录的流式
/// 聚合同一形态）；其余事件按序插为引用行。
pub fn render_markdown(title: &str, session_id: &str, events: &[AgentEvent]) -> String {
    let mut out = format!(
        "# {title}\n\n> 导出自 Locowrk · 会话 `{session_id}` · 共 {} 条记录\n\n",
        events.len()
    );
    let mut assistant = String::new();
    for e in events {
        match e {
            AgentEvent::Chunk { delta, .. } => assistant.push_str(delta),
            AgentEvent::UserMessage { content, parts, .. } => {
                section(&mut out, "## 助手\n\n", &assistant);
                assistant.clear();
                // parts（批 D）折叠为占位行——导出是面向人的 Markdown，
                // 附件只留名目（无损归档用 jsonl 原件）
                section(&mut out, "## 用户\n\n", &session_store::fold_parts(content, parts));
            }
            AgentEvent::ToolCall { tool, args, .. } => {
                section(&mut out, "## 助手\n\n", &assistant);
                assistant.clear();
                out.push_str(&format!(
                    "**工具 · {tool}**\n\n```\n{}\n```\n\n",
                    clip_json(args)
                ));
            }
            AgentEvent::ToolResult {
                ok, output, error, ..
            } => {
                section(&mut out, "## 助手\n\n", &assistant);
                assistant.clear();
                if let Some(err) = error {
                    out.push_str(&format!("> 工具结果 · 失败：{}\n\n", clip(err)));
                } else if *ok {
                    out.push_str(&format!("> 工具结果 · 成功：{}\n\n", clip_json(output)));
                } else {
                    out.push_str("> 工具结果 · 失败\n\n");
                }
            }
            AgentEvent::ApprovalResolved { tool, allowed, .. } => {
                section(&mut out, "## 助手\n\n", &assistant);
                assistant.clear();
                let word = if *allowed { "已允许" } else { "已拒绝" };
                out.push_str(&format!("> 审批记录：{word} · {tool}\n\n"));
            }
            AgentEvent::ContextCompacted {
                summary_preview, ..
            } => {
                section(&mut out, "## 助手\n\n", &assistant);
                assistant.clear();
                out.push_str(&format!("> 上下文已压缩：{}\n\n", clip(summary_preview)));
            }
            _ => {}
        }
    }
    section(&mut out, "## 助手\n\n", &assistant);
    out
}

#[cfg(test)]
mod tests;
