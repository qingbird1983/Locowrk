//! 任务选择卡后端（v4.11 §3.39①，UI shell 文档）：`ask_user` 工具的反向
//! 请求通道——模型在需要用户裁决时调用，工具执行即把卡片载荷推给前端并
//! 挂起等待应答。
//!
//! 与 `ApprovalHub` 同款机械件（oneshot 等待 + 守夜超时 + 事件推送），差异
//! 只在语义：应答是**内容**不是授权，`Pure` 工具不走审批门；超时按**跳过**
//! 收束（轮次不永久挂死，可配置 `tools.askTimeoutSecs`，0 = 不限）；用户在
//! 卡内首次交互可 `ask_snooze` 续期（摘除守夜，防"打字打到一半被超时吞"）。
//! 停止生成（§3.39④）时 `skip_session` 把该会话全部待决按跳过收束。
//!
//! 请求/等待不进会话日志——日志里的 `ToolCall/ToolResult ask_user` 由内核
//! 落（纪律 4/6），卡片只是"问人"的实时通道（与 ApprovalRequest 同款纪律）。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use serde_json::{json, Value};
use tokio::sync::oneshot;

use crate::events::{AgentEvent, AskOptionPayload, EventSink};

/// 守夜缺省（可配置原则：生产初值来自设置 `tools.askTimeoutSecs`，经
/// [`AskHub::set_timeout_secs`] 热换入；0 = 不限）。
pub const ASK_TIMEOUT_DEFAULT_SECS: u64 = 300;

/// 一次应答：选择（labels + 自由补充）或跳过（含超时/停止的自动跳过）。
#[derive(Debug, Clone)]
pub enum AskAnswer {
    Answer { labels: Vec<String>, note: String },
    Skipped { reason: Option<String> },
}

struct PendingAsk {
    tx: oneshot::Sender<AskAnswer>,
    session_id: Option<String>,
    /// 登记时的完整载荷事件：`list_pending_asks` 补拉用（pending 表只存
    /// tx 的话载荷无法还原）。
    cached_event: Option<AgentEvent>,
}

pub struct AskHub {
    sink: Arc<dyn EventSink>,
    pending: Arc<Mutex<HashMap<String, PendingAsk>>>,
    /// 已续期（用户开始交互）的请求：守夜到点先查这里，命中则不收束。
    snoozed: Arc<Mutex<HashSet<String>>>,
    timeout_ms: AtomicU64,
    seq: AtomicU64,
}

impl AskHub {
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        Self {
            sink,
            pending: Arc::new(Mutex::new(HashMap::new())),
            snoozed: Arc::new(Mutex::new(HashSet::new())),
            timeout_ms: AtomicU64::new(ASK_TIMEOUT_DEFAULT_SECS.saturating_mul(1000)),
            seq: AtomicU64::new(1),
        }
    }

    /// 守夜超时热换入（settings_set 保存即生效）；0 = 不限等待。
    pub fn set_timeout_secs(&self, secs: u64) {
        self.timeout_ms
            .store(secs.saturating_mul(1000), Ordering::Relaxed);
    }

    fn timeout_secs(&self) -> u64 {
        self.timeout_ms.load(Ordering::Relaxed) / 1000
    }

    /// `list_pending_asks`：前端刷新/重连时补拉未决请求。
    pub fn pending(&self) -> Vec<AgentEvent> {
        // 返回完整 AskUser 事件形状，前端与实时通道同一消费口。
        // pending 表只存 tx（载荷不冗余）——补拉无法还原 question/options，
        // 因此登记时把事件缓存进表（见 request）。
        self.pending
            .lock()
            .expect("ask pending poisoned")
            .values()
            .filter_map(|p| p.cached_event.clone())
            .collect()
    }

    /// `ask_respond`：回填应答/跳过并唤醒挂起的工具调用。请求不存在
    /// （已处理 / 超时已被守夜跳过）返回 Err。
    pub fn respond(&self, request_id: &str, answer: AskAnswer) -> Result<(), String> {
        let entry = self
            .pending
            .lock()
            .expect("ask pending poisoned")
            .remove(request_id);
        let Some(entry) = entry else {
            return Err(format!("未知或已处理的选择请求：{request_id}"));
        };
        let sid = entry.session_id.clone();
        let _ = entry.tx.send(answer);
        self.sink.emit(&AgentEvent::AskResolved {
            session_id: sid,
            request_id: request_id.into(),
        });
        Ok(())
    }

    /// `ask_snooze`（§3.39①）：用户在卡内首次交互 → 守夜摘除（本次等待
    /// 不再超时；请求已收束 = 空转幂等）。
    pub fn snooze(&self, request_id: &str) {
        self.snoozed
            .lock()
            .expect("ask snoozed poisoned")
            .insert(request_id.to_string());
    }

    /// 停止生成 / 会话收尾：该会话（含子 Agent `{parent}-sub-{n}`）全部
    /// 待决按跳过收束——不留悬空卡，也不让停止后的轮次继续挂在问题上。
    pub fn skip_session(&self, session_id: &str) {
        let targets: Vec<String> = self
            .pending
            .lock()
            .expect("ask pending poisoned")
            .iter()
            .filter(|(_, p)| {
                p.session_id.as_deref().is_some_and(|sid| {
                    sid == session_id || sid.starts_with(&format!("{session_id}-sub-"))
                })
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in targets {
            let _ = self.respond(
                &id,
                AskAnswer::Skipped {
                    reason: Some("用户停止了任务".into()),
                },
            );
        }
    }

    /// 工具执行入口：推卡片 → 等待 → 应答。`session_id` 来自 CallCtx
    /// （子 Agent 派生调用带 `{parent}-sub-{n}`，前端打徽章）。
    async fn request(
        &self,
        session_id: Option<&str>,
        question: &str,
        options: Vec<AskOptionPayload>,
        multi_select: bool,
    ) -> AskAnswer {
        let request_id = format!("ask-{}", self.seq.fetch_add(1, Ordering::Relaxed));
        let event = AgentEvent::AskUser {
            session_id: session_id.map(str::to_string),
            request_id: request_id.clone(),
            question: question.to_string(),
            options: options.clone(),
            multi_select,
            timeout_secs: self.timeout_secs(),
        };
        let (tx, rx) = oneshot::channel();
        self.pending.lock().expect("ask pending poisoned").insert(
            request_id.clone(),
            PendingAsk {
                tx,
                session_id: session_id.map(str::to_string),
                cached_event: Some(event.clone()),
            },
        );

        self.sink.emit(&event);

        // 守夜（可配置，0 = 不限）：到点且未被 snooze → 按跳过收束；
        // respond 先到先清（表里摘掉了，此处空转）。守夜自带 sink 句柄，
        // 收束时自己发 AskResolved（四路摘卡同源：应答/跳过经 respond、
        // 超时经这里、停止经 skip_session→respond）。
        let timeout_ms = self.timeout_ms.load(Ordering::Relaxed);
        if timeout_ms > 0 {
            let pending = Arc::clone(&self.pending);
            let snoozed = Arc::clone(&self.snoozed);
            let sink = Arc::clone(&self.sink);
            let rid = request_id.clone();
            let sid = session_id.map(str::to_string);
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(timeout_ms)).await;
                if snoozed.lock().expect("ask snoozed poisoned").remove(&rid) {
                    return; // 用户正在作答，不吞草稿
                }
                let entry = pending.lock().expect("ask pending poisoned").remove(&rid);
                if let Some(entry) = entry {
                    let _ = entry.tx.send(AskAnswer::Skipped {
                        reason: Some(format!("超时自动跳过（{} 秒未回答）", timeout_ms / 1000)),
                    });
                    sink.emit(&AgentEvent::AskResolved {
                        session_id: sid,
                        request_id: rid,
                    });
                }
            });
        }

        match rx.await {
            // 主动应答路（respond / skip_session / 守夜）各自已发
            // AskResolved；这里只等结果。通道关闭（Hub 被丢弃）= 跳过收束。
            Ok(answer) => answer,
            Err(_) => AskAnswer::Skipped {
                reason: Some("选择通道已关闭（请求被丢弃）".into()),
            },
        }
    }
}

/// 内置工具 `ask_user`（§3.39①）：`Pure` + `always`——审批门直接放行、
/// 常驻目录（模型随时可问）。参数：question 必填；options 可选（label 必填
/// + description 可选）；multi_select 可选。
pub struct AskUserTool {
    hub: Arc<AskHub>,
}

impl AskUserTool {
    pub fn new(hub: Arc<AskHub>) -> Self {
        Self { hub }
    }
}

fn spec() -> &'static ToolSpec {
    static SPEC: std::sync::OnceLock<ToolSpec> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "ask_user".into(),
        description: "Ask the user a blocking question and wait for their answer. Use this \
                      when a decision needs the user's input (choose an option or free-form \
                      reply). The question is shown as an interactive card; provide 2-4 \
                      concise options when possible."
            .into(),
        schema: json!({
            "type": "object",
            "properties": {
                "question": {
                    "type": "string",
                    "description": "The question to ask, phrased so the user can answer it directly"
                },
                "options": {
                    "type": "array",
                    "description": "Optional predefined choices; omit for a free-form reply",
                    "items": {
                        "type": "object",
                        "properties": {
                            "label": { "type": "string", "description": "Short option text" },
                            "description": { "type": "string", "description": "Optional one-line explanation" }
                        },
                        "required": ["label"]
                    }
                },
                "multi_select": {
                    "type": "boolean",
                    "description": "Allow picking several options at once (default false)"
                }
            },
            "required": ["question"]
        }),
        side_effect: SideEffect::Pure,
        exposure: Exposure::Always,
    })
}

#[async_trait]
impl Tool for AskUserTool {
    fn spec(&self) -> &ToolSpec {
        spec()
    }

    fn plugin_version(&self) -> &Version {
        static V: Version = Version::new(1, 0, 0);
        &V
    }

    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let question = args
            .get("question")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        if question.is_empty() {
            return Err(ToolError::Failed {
                tool: "ask_user".into(),
                message: "question must be a non-empty string".into(),
            });
        }
        let raw_options = args
            .get("options")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        if raw_options.len() > 8 {
            return Err(ToolError::Failed {
                tool: "ask_user".into(),
                message: "at most 8 options per question".into(),
            });
        }
        let mut options = Vec::with_capacity(raw_options.len());
        for o in &raw_options {
            let label = o
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            if label.is_empty() {
                return Err(ToolError::Failed {
                    tool: "ask_user".into(),
                    message: "every option needs a non-empty label".into(),
                });
            }
            options.push(AskOptionPayload {
                description: o
                    .get("description")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                label,
            });
        }
        let multi_select = args
            .get("multi_select")
            .and_then(Value::as_bool)
            .unwrap_or(false);

        match self
            .hub
            .request(ctx.session_id.as_deref(), &question, options, multi_select)
            .await
        {
            AskAnswer::Answer { labels, note } => {
                let mut out = json!({ "answer": labels });
                if !note.is_empty() {
                    out["note"] = Value::String(note);
                }
                Ok(out)
            }
            AskAnswer::Skipped { reason } => match reason {
                Some(r) => Ok(json!({ "skipped": true, "reason": r })),
                None => Ok(json!({ "skipped": true })),
            },
        }
    }
}
