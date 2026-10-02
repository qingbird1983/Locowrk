//! 子 Agent 运行循环（⑮）：fork 父转录前缀 → 追加任务 → 循环
//! `step_as` 到收敛 → 摘要回父。轮数/超时越限**如实收束**返回部分
//! 结果（status 标明，摘要取已有转录尾部，不虚构完成）。

use std::time::Duration;

use serde_json::{Value, json};
use session_store::{Session, SessionEvent};

use super::SubagentGate;
use crate::kernel::Kernel;

/// 缺省/上限预算（工具层钳制输入，这里是唯一事实源）。
pub const DEFAULT_MAX_TURNS: u32 = 8;
pub const MAX_TURNS_CAP: u32 = 32;
pub const DEFAULT_TIMEOUT_SECS: u64 = 300;
pub const TIMEOUT_CAP_SECS: u64 = 1800;
/// 回给父的摘要上限（字符）：父要读它，全文留在子转录里。
pub const SUMMARY_CLIP: usize = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentStatus {
    Completed,
    BudgetExhausted,
    Timeout,
    Failed,
}

impl SubagentStatus {
    fn as_str(&self) -> &'static str {
        match self {
            SubagentStatus::Completed => "completed",
            SubagentStatus::BudgetExhausted => "budget_exhausted",
            SubagentStatus::Timeout => "timeout",
            SubagentStatus::Failed => "failed",
        }
    }
}

/// 一次子 Agent 运行的收束（工具输出 = `to_json()`，LLM 与父会话日志
/// 看到同一结构）。
#[derive(Debug, Clone)]
pub struct SubagentOutcome {
    pub status: SubagentStatus,
    pub summary: String,
    pub turns: u32,
    pub tools_used: Vec<String>,
    pub child_session_id: String,
    pub error: Option<String>,
}

impl SubagentOutcome {
    pub fn to_json(&self) -> Value {
        let mut obj = json!({
            "status": self.status.as_str(),
            "summary": self.summary,
            "turns": self.turns,
            "tools_used": self.tools_used,
            "child_session_id": self.child_session_id,
        });
        if let (Some(err), Some(slot)) = (&self.error, obj.as_object_mut()) {
            slot.insert("error".into(), json!(err));
        }
        obj
    }
}

enum LoopEnd {
    Converged,
    Budget,
    Timeout,
    Step(String),
}

/// 跑一个子 Agent。`allow` 语义见 FilterToolSource（空 = 继承父全目录）。
/// [`run_child`] 的入参束（8 参触发 clippy too_many_arguments，收拢）。
pub(crate) struct ChildRunParams<'a> {
    pub parent: &'a Kernel,
    pub gate: &'a SubagentGate,
    pub parent_session_id: &'a str,
    pub prompt: String,
    pub allow: Vec<String>,
    pub max_turns: u32,
    pub timeout: Option<Duration>,
    pub transcript_sink: Option<&'a dyn crate::subagent::SubTranscriptSink>,
}

pub(crate) async fn run_child(
    ChildRunParams {
        parent,
        gate,
        parent_session_id,
        prompt,
        allow,
        max_turns,
        timeout,
        transcript_sink,
    }: ChildRunParams<'_>,
) -> SubagentOutcome {
    let pre = |error: String| SubagentOutcome {
        status: SubagentStatus::Failed,
        summary: String::new(),
        turns: 0,
        tools_used: Vec::new(),
        child_session_id: String::new(),
        error: Some(error),
    };

    // 父转录前缀（⑨ fork 语义：可见事件克隆，游标前投影）
    let Some(prefix) = gate.snapshot(parent_session_id) else {
        return pre(format!(
            "父会话 `{parent_session_id}` 没有转录快照（会话未在本内核走过 step）"
        ));
    };
    let child_id = gate.next_child_id(parent_session_id);
    let child_kernel = match parent.child_kernel(allow) {
        Ok(k) => k,
        Err(e) => return pre(format!("子 Agent 工具视图构建失败：{e}")),
    };

    let mut child = prefix.fork(prefix.cursor());
    // 子"自身"段从任务 prompt **之前**起算：prompt 是子的自有内容（§3.16
    // 落盘/页签重放以任务开头），fork 来的父前缀（父的历史 ToolCall/
    // AssistantMessage）被排除——收束汇报（tools_used/摘要）只看子内新增
    // 段，父的调用史不会被误报成子的工具使用（㋑·fix 同一语义）。
    let own_from = child.len();
    child.append(SessionEvent::UserMessage { content: prompt });

    let mut turns: u32 = 0;
    let loop_fut = async {
        let mut end = LoopEnd::Budget;
        for turn in 1..=max_turns {
            if let Err(e) = child_kernel.step_as(&mut child, &child_id).await {
                end = LoopEnd::Step(e.to_string());
                break;
            }
            turns = turn;
            if converged(&child) {
                end = LoopEnd::Converged;
                break;
            }
        }
        end
    };
    let end = match timeout {
        Some(d) => match tokio::time::timeout(d, loop_fut).await {
            Ok(end) => end,
            Err(_) => LoopEnd::Timeout,
        },
        None => loop_fut.await,
    };

    let (status, error) = match end {
        LoopEnd::Converged => (SubagentStatus::Completed, None),
        LoopEnd::Budget => (SubagentStatus::BudgetExhausted, None),
        LoopEnd::Timeout => (SubagentStatus::Timeout, None),
        LoopEnd::Step(e) => (SubagentStatus::Failed, Some(e)),
    };

    // §3.16：子自身段交给宿主落盘（可注入；None = 纯黑盒）。fork 失败等
    // 早退路径 child_session_id 为空，无转录可交。
    if let Some(sink) = transcript_sink {
        sink.close(&child_id, own_from, &child);
    }

    SubagentOutcome {
        status,
        summary: tail_summary(&child, own_from),
        turns,
        tools_used: tools_used(&child, own_from),
        child_session_id: child_id,
        error,
    }
}

/// 收敛判据（同桌面 runner）：最后一条 assistant 不再带工具调用。
fn converged(child: &Session) -> bool {
    matches!(
        child.events().last(),
        Some(SessionEvent::AssistantMessage { tool_calls, .. }) if tool_calls.is_empty()
    )
}

/// 摘要 = 子转录**自身段**（`own_from` 之后）最后一条非空 assistant 正文；
/// 没有（纯工具调用步被截断/超时）就如实描述最后动作，不虚构进展。
fn tail_summary(child: &Session, own_from: usize) -> String {
    for e in child.events()[own_from..].iter().rev() {
        if let SessionEvent::AssistantMessage { content, .. } = e {
            if !content.is_empty() {
                return content.chars().take(SUMMARY_CLIP).collect();
            }
        }
    }
    match tools_used(child, own_from).last() {
        Some(t) => format!("（未产出文本；最后动作：调用 {t}）"),
        None => "（未产出任何内容）".into(),
    }
}

/// 子内调用过的工具（去重保序）——只扫自身段，fork 前缀里父的历史调用
/// 不算子的使用。
fn tools_used(child: &Session, own_from: usize) -> Vec<String> {
    let mut used: Vec<String> = Vec::new();
    for e in &child.events()[own_from..] {
        if let SessionEvent::ToolCall { tool, .. } = e {
            if !used.iter().any(|u| u == tool) {
                used.push(tool.clone());
            }
        }
    }
    used
}
