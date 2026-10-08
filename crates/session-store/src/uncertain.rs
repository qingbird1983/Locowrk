//! Unknown Effect 扫描与诚实闭合（G2，可靠性稿 §5）。
//!
//! 纪律 5「先写 ToolCall 再执行」保证崩溃后日志里能识别悬空调用：
//! AssistantMessage 声明了 call_id 但其后没有 ToolResult 事件 = 进程在
//! 执行中退出，副作用未知（可能已执行 / 部分执行 / 未执行，无法从本地
//! 证据判断——§5.1）。本模块是扫描（判定谁悬空、谁已被裁决）与合成
//! 闭合（投影层的"效果未知 / 用户裁决"载荷）的单点实现：
//!
//! - 消息投影（`session::project_messages`）用它给悬空调用合成
//!   ToolResult 消息——不给合成，下一轮请求带悬空 tool_calls 会被
//!   供应商整单拒收（㉘·B 走查实抓的形状）；
//! - 桌面投影 / 裁决命令（runner::project_events / commands）用它
//!   判定前端 uncertain 卡与命令合法性。
//!
//! 合成只发生在投影层，日志不回填 ToolResult——事件只追加真实发生的
//! 事，"效果未知"与用户裁决都是可随时重推导的投影事实。

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::{Value, json};

use plugin_api::CallId;

use crate::event::{SessionEvent, UncertainDecision};

/// 一个悬空调用（未闭合且未裁决）。`tool`/`args` 可能缺：AssistantMessage
/// 声明了两个调用、进程在第二个 ToolCall 事件落盘前退出——第二个只有
/// 声明位，细节不可知（前端如实渲染"细节缺失"）。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct UncertainCall {
    pub call_id: CallId,
    pub tool: String,
    pub args: Value,
}

/// 悬空扫描结果（单遍产物，三个消费面各取所需）。
#[derive(Debug, Clone, Default)]
pub struct UncertainScan {
    /// 全部悬空 id（声明了但没有 ToolResult 事件，**含已裁决**——
    /// 裁决不回填 ToolResult，悬空是日志事实）。
    pub dangling: HashSet<String>,
    /// 裁决表：悬空 id → (裁决, note)。只含 ToolExecutionUncertain
    /// 覆盖过的 id。
    pub decisions: HashMap<String, (UncertainDecision, String)>,
}

impl UncertainScan {
    /// 悬空且**未**裁决（前端 uncertain 卡的数据源，按 assistant 声明序）。
    /// 以声明位为基准（不是 ToolCall 事件）——进程死在两次落盘之间的
    /// 调用只有声明位，也要如实入列（tool/args 回填自 ToolCall 事件，
    /// 缺席 = 细节不可知）。
    pub fn unresolved(&self, events: &[SessionEvent]) -> Vec<UncertainCall> {
        let mut out: Vec<UncertainCall> = Vec::new();
        let mut seen = HashSet::new();
        for e in events {
            if let SessionEvent::AssistantMessage { tool_calls, .. } = e {
                for id in tool_calls {
                    if self.dangling.contains(&id.0)
                        && !self.decisions.contains_key(&id.0)
                        && seen.insert(id.0.clone())
                    {
                        out.push(UncertainCall {
                            call_id: id.clone(),
                            tool: String::new(),
                            args: Value::Null,
                        });
                    }
                }
            }
        }
        for item in &mut out {
            for e in events {
                if let SessionEvent::ToolCall {
                    call_id,
                    tool,
                    args,
                    ..
                } = e
                    && call_id.0 == item.call_id.0
                {
                    item.tool.clone_from(tool);
                    item.args.clone_from(args);
                }
            }
        }
        out
    }
}

/// 悬空扫描（单遍）：AssistantMessage 声明的 call_id 集合 − 有 ToolResult
/// 事件的集合 = 悬空；ToolExecutionUncertain 收进裁决表。
pub fn scan_uncertain(events: &[SessionEvent]) -> UncertainScan {
    let mut declared: HashSet<String> = HashSet::new();
    let mut closed: HashSet<String> = HashSet::new();
    let mut decisions: HashMap<String, (UncertainDecision, String)> = HashMap::new();
    for e in events {
        match e {
            SessionEvent::AssistantMessage { tool_calls, .. } => {
                for id in tool_calls {
                    declared.insert(id.0.clone());
                }
            }
            SessionEvent::ToolResult { call_id, .. } => {
                closed.insert(call_id.0.clone());
            }
            SessionEvent::ToolExecutionUncertain {
                call_id,
                decision,
                note,
                ..
            } => {
                decisions.insert(call_id.0.clone(), (*decision, note.clone()));
            }
            _ => {}
        }
    }
    UncertainScan {
        dangling: declared.difference(&closed).cloned().collect(),
        decisions,
    }
}

/// 悬空调用的诚实闭合载荷（模型面 / UI 行明细共用同一措辞，单点）。
/// `decision = None`（未裁决）= 效果未知——不是失败也不是成功，模型被
/// 明确告知先核实实际状态再依赖结果（"不确定的副作用永远不自动重试"
/// 的模型侧表达：重试是模型看过闭合后的自主决定，走全部既有门）。
pub fn closure(decision: Option<(UncertainDecision, &str)>) -> (Value, Option<String>) {
    let (mut obj, err) = match decision {
        None => (
            json!({
                "status": "unknown",
                "detail": "the process exited while this call was executing; whether it took effect is unknown. verify the actual state before relying on it"
            }),
            None,
        ),
        Some((UncertainDecision::Completed, _note)) => (
            json!({
                "status": "confirmed_completed",
                "detail": "user confirmed after restart that this call's effect happened"
            }),
            None,
        ),
        Some((UncertainDecision::NotExecuted, _note)) => (
            json!({
                "status": "confirmed_not_executed",
                "detail": "user confirmed after restart that this call did not take effect; redoing it is safe"
            }),
            None,
        ),
        Some((UncertainDecision::Inspected, note)) => (
            json!({
                "status": "inspected",
                "detail": format!("user inspected the actual state after restart and recorded: {note}"),
            }),
            None,
        ),
    };
    // 用户附言：有才带字段（宁缺勿假，不落 null 键）
    if let Some((_, note)) = decision
        && !note.is_empty()
        && !matches!(decision, Some((UncertainDecision::Inspected, _)))
    {
        obj["note"] = Value::String(note.to_string());
    }
    (obj, err)
}

#[cfg(test)]
mod tests;
