//! 子 Agent 运行循环（⑮）：fork 父转录前缀 → 追加任务 → 循环
//! `step_as` 到收敛 → 摘要回父。轮数/超时越限**如实收束**返回部分
//! 结果（status 标明，摘要取已有转录尾部，不虚构完成）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};
use session_store::{Session, SessionEvent, TurnId, TurnOutcome};

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
/// 看到同一结构）。`model` / `model_failures`（批 F）= 子代理模型面
/// 报告：末用模型与失败/跳过记因；无模型面（未接工厂）两值皆空。
#[derive(Debug, Clone)]
pub struct SubagentOutcome {
    pub status: SubagentStatus,
    pub summary: String,
    pub turns: u32,
    pub tools_used: Vec<String>,
    pub child_session_id: String,
    pub error: Option<String>,
    pub model: Option<String>,
    pub model_failures: Vec<String>,
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
        if let (Some(model), Some(slot)) = (&self.model, obj.as_object_mut()) {
            slot.insert("model".into(), json!(model));
        }
        if let Some(slot) = obj.as_object_mut() {
            if !self.model_failures.is_empty() {
                slot.insert("model_failures".into(), json!(self.model_failures));
            }
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
    /// 子代理模型面工厂（批 F）：Some = 每次调用造「带回退链的服务 + 报
    /// 告」换入子内核作用域；None = 全局服务（⑮ 以来行为不变）。
    pub child_model: Option<&'a crate::subagent::ChildModelFactory>,
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
        child_model,
    }: ChildRunParams<'_>,
) -> SubagentOutcome {
    let pre = |error: String| SubagentOutcome {
        status: SubagentStatus::Failed,
        summary: String::new(),
        turns: 0,
        tools_used: Vec::new(),
        child_session_id: String::new(),
        error: Some(error),
        model: None,
        model_failures: Vec::new(),
    };

    // 父转录前缀（⑨ fork 语义：可见事件克隆，游标前投影）
    let Some(prefix) = gate.snapshot(parent_session_id) else {
        return pre(format!(
            "父会话 `{parent_session_id}` 没有转录快照（会话未在本内核走过 step）"
        ));
    };
    let child_id = gate.next_child_id(parent_session_id);
    // 批 F：每 run 造一个模型面（带回退链的服务 + 报告句柄），报告随
    // 收束进 outcome；无工厂 = None（headless / 未接组装点）。
    let model_handle = child_model.map(|f| f());
    let child_kernel =
        match parent.child_kernel(allow, model_handle.as_ref().map(|h| Arc::clone(&h.service))) {
            Ok(k) => k,
            Err(e) => return pre(format!("子 Agent 工具视图构建失败：{e}")),
        };

    let mut child = prefix.fork(prefix.cursor());
    // 子"自身"段从任务 prompt **之前**起算：prompt 是子的自有内容（§3.16
    // 落盘/页签重放以任务开头），fork 来的父前缀（父的历史 ToolCall/
    // AssistantMessage）被排除——收束汇报（tools_used/摘要）只看子内新增
    // 段，父的调用史不会被误报成子的工具使用（㋑·fix 同一语义）。
    let own_from = child.len();
    // 轮界时刻与主对话同款（v2.101）：子转录文件此前没有任何时刻，页签
    // 计时行按「无时刻不虚构」只剩发丝线（v2.107 目检实抓）。子只有任务
    // prompt 一个用户轮，turn_id 恒 1（fork 前缀里的父 TurnStart 不算子的）。
    child.append(SessionEvent::TurnStart {
        turn_id: TurnId(1),
        ts: now_ms(),
    });
    child.append(SessionEvent::UserMessage {
        content: prompt,
        parts: Vec::new(),
    });

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

    // 收束即落 TurnEnd（四路径全覆盖，同主对话 finish_turn 纪律）：预算
    // 到顶是优雅收束（Completed，同主对话步数到顶），超时/步失败如实
    // Failed——重放才知道轮停在哪，计时行据此冻结读数。outcome 先算
    // （Copy），status 匹配要搬走 Step 的错误串。
    let outcome = match end {
        LoopEnd::Converged | LoopEnd::Budget => TurnOutcome::Completed,
        LoopEnd::Timeout | LoopEnd::Step(_) => TurnOutcome::Failed,
    };
    child.append(SessionEvent::TurnEnd {
        turn_id: TurnId(1),
        outcome,
        ts: now_ms(),
    });

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

    // 批 F：模型面报告（末用模型 + 失败/跳过记因）随收束回父。
    let (model, model_failures) = match &model_handle {
        Some(h) => h.report.snapshot(),
        None => (None, Vec::new()),
    };

    SubagentOutcome {
        status,
        summary: tail_summary(&child, own_from),
        turns,
        tools_used: tools_used(&child, own_from),
        child_session_id: child_id,
        error,
        model,
        model_failures,
    }
}

/// 收敛判据（同桌面 runner，v2.64 修法）：从尾部找**最近一条**
/// AssistantMessage 判 tool_calls——不能只看 last()：报告用量的供应商
/// step 后 last() 恒为 LlmUsage（内核紧贴 assistant 落），子循环永不
/// 收敛、白跑到轮数上限，多余的一步还可能吃到 thinking 模型对 assistant
/// 结尾请求的 400（盘证：现网子代理收束 status 恒 budget_exhausted/failed）。
fn converged(child: &Session) -> bool {
    matches!(
        child.events().iter().rev().find_map(|e| match e {
            SessionEvent::AssistantMessage { tool_calls, .. } => Some(tool_calls.is_empty()),
            _ => None,
        }),
        Some(true)
    )
}

/// Unix 毫秒（kernel.rs ContextCompacted 同款口径；时钟不可用 = 0，
/// 投影按缺失处理不虚构）。
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
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
