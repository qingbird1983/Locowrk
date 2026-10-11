//! 子代理独立验收（G5，可靠性稿 §7 随批 F）：宿主内置 Pure + OnDemand
//! 工具。子说「完成了」不算完成——父会话用它对子转录做**独立验收**：
//! 读 `sub-transcripts/<id>.jsonl` 紧凑转录 → 验收 LLM 调用（吃辅助降档，
//! `params["aux"]=true` 批 B §5.4 通道）→ 逐准则裁决。
//!
//! claimed/verified 分离在既有工具结果通道表达：`subagent_run` 输出 =
//! claimed，本工具输出 = verified（设计稿 §7.6 偏离记档：不设新
//! SessionEvent——验收结果就是父会话里的一次工具结果，双轨事件无增益）。

use std::path::PathBuf;
use std::sync::Arc;

use crate::text::clip_ellipsis;
use async_trait::async_trait;
use kernel_agent::{LlmRequest, LlmService};
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{json, Value};
use session_store::{fold_parts, load_session, session_file, Message, SessionEvent};

/// 组装点固定的版本身份（内置工具无插件版本，占位 0.0.0）。
static TOOL_VERSION: semver::Version = semver::Version::new(0, 0, 0);

/// 单消息裁剪（转录是给验收员看的证据，不是全文回放）。
const MSG_CLIP: usize = 2000;
/// 转录总顶（辅助调用便宜，但别无限）。
const TRANSCRIPT_CAP: usize = 60_000;

pub struct SubagentVerifyTool {
    llm: Arc<dyn LlmService>,
    dir: PathBuf,
    spec: ToolSpec,
}

impl SubagentVerifyTool {
    pub fn new(llm: Arc<dyn LlmService>, dir: PathBuf) -> Self {
        Self {
            llm,
            dir,
            spec: ToolSpec {
                name: "subagent_verify".into(),
                description: "对子 Agent（subagent_run）的执行结果做独立验收：读子会话转录，\
                    逐条核对验收条件，返回 verified / failed / inconclusive 与证据。\
                    子代理自述完成不等于完成——重要任务收束后应调用本工具核验。\
                    参数：subagent_id（必填，subagent_run 返回的 child_session_id）；\
                    acceptance_criteria（必填，验收条件字符串数组，如 [\"测试通过\",\"文件已修改\"]）。"
                    .into(),
                schema: json!({
                    "type": "object",
                    "properties": {
                        "subagent_id": { "type": "string", "description": "subagent_run 返回的 child_session_id" },
                        "acceptance_criteria": {
                            "type": "array",
                            "items": { "type": "string" },
                            "description": "验收条件列表"
                        }
                    },
                    "required": ["subagent_id", "acceptance_criteria"]
                }),
                side_effect: SideEffect::Pure,
                exposure: Exposure::OnDemand,
            },
        }
    }
}

fn tool_err(message: impl Into<String>) -> ToolError {
    ToolError::Failed {
        tool: "subagent_verify".into(),
        message: message.into(),
    }
}

/// 紧凑转录：证据行（任务 / 助手正文 / 工具调用与结果），逐消息与总量
/// 双裁剪。Thinking / 用量 / 轮界不进（不是完成度的证据）。
fn build_transcript(events: &[SessionEvent]) -> String {
    let mut out = String::new();
    let mut push = |line: String| {
        if out.len() >= TRANSCRIPT_CAP {
            return;
        }
        out.push_str(&line);
        out.push('\n');
    };
    for e in events {
        match e {
            SessionEvent::UserMessage { content, parts } => {
                let text = clip_ellipsis(&fold_parts(content, parts), MSG_CLIP);
                push(format!("[任务] {text}"));
            }
            SessionEvent::AssistantMessage {
                content,
                tool_calls,
            } => {
                if !content.is_empty() {
                    push(format!("[助手] {}", clip_ellipsis(content, MSG_CLIP)));
                }
                if !tool_calls.is_empty() {
                    push(format!("[助手] （发起 {} 个工具调用）", tool_calls.len()));
                }
            }
            SessionEvent::ToolCall {
                call_id,
                tool,
                args,
                ..
            } => {
                let a = clip_ellipsis(&args.to_string(), 200);
                push(format!("[调用 {}] {tool}({a})", call_id.0));
            }
            SessionEvent::ToolResult {
                call_id,
                output,
                error,
            } => match error {
                Some(err) => push(format!(
                    "[结果 {}] 失败: {}",
                    call_id.0,
                    clip_ellipsis(err, 400)
                )),
                None => push(format!(
                    "[结果 {}] {}",
                    call_id.0,
                    clip_ellipsis(&output.to_string(), 400)
                )),
            },
            _ => {}
        }
    }
    if out.len() >= TRANSCRIPT_CAP {
        out.push_str("[转录过长，已截断]\n");
    }
    out
}

/// 宽松解析：剥一层代码围栏后取首个 `{…}` 平衡块；解析失败 =
/// inconclusive + 原文截断（如实，不发明裁决）。
fn parse_verdict(raw: &str, criteria: &[String]) -> (String, Value, String) {
    let text = raw.trim();
    let text = text
        .strip_prefix("```json")
        .or_else(|| text.strip_prefix("```"))
        .unwrap_or(text)
        .trim();
    let candidate = match (text.find('{'), text.rfind('}')) {
        (Some(a), Some(b)) if b > a => &text[a..=b],
        _ => text,
    };
    if let Ok(v) = serde_json::from_str::<Value>(candidate) {
        let status = v
            .get("status")
            .and_then(Value::as_str)
            .filter(|s| matches!(*s, "verified" | "failed" | "inconclusive"))
            .unwrap_or("inconclusive")
            .to_string();
        let summary = v
            .get("summary")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        return (
            status,
            v.get("criteria").cloned().unwrap_or(Value::Null),
            summary,
        );
    }
    (
        "inconclusive".into(),
        json!(criteria
            .iter()
            .map(|c| json!({
                "criterion": c,
                "verdict": "inconclusive",
                "evidence": "验收输出无法解析"
            }))
            .collect::<Vec<_>>()),
        clip_ellipsis(raw, 800),
    )
}

#[async_trait]
impl Tool for SubagentVerifyTool {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn plugin_version(&self) -> &semver::Version {
        &TOOL_VERSION
    }

    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let obj = args
            .as_object()
            .ok_or_else(|| tool_err("参数必须是对象 {subagent_id, acceptance_criteria}"))?;
        let subagent_id = obj
            .get("subagent_id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| tool_err("subagent_id 必填（subagent_run 返回的 child_session_id）"))?;
        let criteria: Vec<String> = match obj.get("acceptance_criteria") {
            Some(Value::Array(list)) if !list.is_empty() => list
                .iter()
                .filter_map(|v| v.as_str().map(str::trim).filter(|s| !s.is_empty()))
                .map(str::to_string)
                .collect(),
            _ => return Err(tool_err("acceptance_criteria 必填且为非空字符串数组")),
        };
        if criteria.iter().all(|c| c.is_empty()) {
            return Err(tool_err("acceptance_criteria 不能全为空"));
        }

        // id 走与 sessions/ 同一白名单校验（防穿越）；文件缺席 = 子代理
        // 还没收束或 id 有误，如实报错。
        let log: PathBuf = session_file(&self.dir, subagent_id)
            .map_err(|e| tool_err(format!("非法子会话 id: {e}")))?;
        if !log.exists() {
            return Err(tool_err(format!(
                "子转录不存在（子代理尚未收束落盘或 id 有误）: {subagent_id}"
            )));
        }
        let (session, skipped) =
            load_session(&log).map_err(|e| tool_err(format!("子转录读取失败: {e}")))?;
        if skipped > 0 {
            tracing::warn!(child = %subagent_id, skipped, "verify: 转录坏行跳过");
        }
        let transcript = build_transcript(session.events());

        // 验收 LLM 调用（辅助降档：方言最低档 + 不发 max_tokens，批 B 通道）。
        let prompt = format!(
            "你是独立验收员。下面是一次子代理执行的任务转录，以及验收条件。\
             逐条核对每条验收条件是否有转录证据支持，不轻信子代理的自述总结。\
             只输出一个 JSON 对象：{{\"status\": \"verified|failed|inconclusive\", \
             \"criteria\": [{{\"criterion\": \"...\", \"verdict\": \"pass|fail|unclear\", \
             \"evidence\": \"转录中的证据或缺失原因\"}}], \"summary\": \"一句话结论\"}}。\
             全部条件有证据支持 = verified；任一条件证伪 = failed；证据不足 = inconclusive。\n\n\
             验收条件：\n{}\n\n任务转录：\n{}",
            criteria
                .iter()
                .enumerate()
                .map(|(i, c)| format!("{}. {c}", i + 1))
                .collect::<Vec<_>>()
                .join("\n"),
            transcript
        );
        let req = LlmRequest {
            messages: vec![Message::user(prompt)],
            params: json!({ "aux": true }),
            session_id: ctx.session_id.clone(),
            ..LlmRequest::default()
        };
        let resp = self
            .llm
            .complete(req)
            .await
            .map_err(|e| tool_err(format!("验收模型调用失败: {e}")))?;

        let (status, criteria_out, summary) = parse_verdict(&resp.content, &criteria);
        Ok(json!({
            "status": status,
            "subagent_id": subagent_id,
            "criteria": criteria_out,
            "summary": summary,
            "method": "llm-review",
        }))
    }
}

#[cfg(test)]
mod tests;
