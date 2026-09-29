//! 副作用回退：补偿计划（DESIGN.md §6.3）。
//!
//! session-store 只产出**纯计划**（不调工具——执行在内核/编排层，
//! 需要 ToolSource）。补偿工具必须幂等，幂等键 = `call_id`（纪律 10）；
//! `CompensatedLedger` 是跨重试的去重闸门。

use std::collections::{HashMap, HashSet};

use plugin_api::{CallId, SideEffect};

use crate::event::SessionEvent;

/// 一条待执行的补偿动作。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compensation {
    /// 调用补偿工具；args 固定携带 call_id 作为幂等键（§6.3）。
    Undo { call_id: CallId, undo_tool: String },
    /// 不可逆：只能人工介入（§6.3）。
    ManualIntervention { call_id: CallId, reason: String },
}

/// 从 `after_seq` 起的事件推导补偿计划（逆序，§6.3）：
/// - 失败的调用不补偿：副作用未必发生，盲目补偿反而制造新副作用
/// - Pure 免费，跳过
/// - Compensable → `Undo`
/// - Irreversible → `ManualIntervention`
/// - 无对应 ToolCall 的结果（悬空调用）不在此补偿，由 replay 的超时回收处理
pub fn plan(events: &[SessionEvent], after_seq: usize) -> Vec<Compensation> {
    let window = &events[after_seq.min(events.len())..];

    // 第一遍：call_id → ToolCall 声明的副作用等级
    let mut calls: HashMap<&CallId, &SideEffect> = HashMap::new();
    for e in window {
        if let SessionEvent::ToolCall {
            call_id,
            side_effect,
            ..
        } = e
        {
            calls.insert(call_id, side_effect);
        }
    }

    // 第二遍：逆序扫描 ToolResult（补偿 = 逆序回退）
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for e in window.iter().rev() {
        if let SessionEvent::ToolResult { call_id, error, .. } = e {
            if error.is_some() || !seen.insert(call_id) {
                continue;
            }
            match calls.get(call_id) {
                Some(SideEffect::Compensable { undo_tool }) => out.push(Compensation::Undo {
                    call_id: call_id.clone(),
                    undo_tool: undo_tool.clone(),
                }),
                Some(SideEffect::Irreversible) => out.push(Compensation::ManualIntervention {
                    call_id: call_id.clone(),
                    reason: "irreversible side effect".into(),
                }),
                _ => {} // Pure：回退免费
            }
        }
    }
    out
}

/// 已执行补偿的登记：跨重试的幂等闸门（纪律 10）。
#[derive(Debug, Default)]
pub struct CompensatedLedger {
    done: HashSet<CallId>,
}

impl CompensatedLedger {
    /// 首次登记返回 true；重试（重复补偿）返回 false。
    pub fn claim(&mut self, call_id: &CallId) -> bool {
        self.done.insert(call_id.clone())
    }

    pub fn contains(&self, call_id: &CallId) -> bool {
        self.done.contains(call_id)
    }

    pub fn len(&self) -> usize {
        self.done.len()
    }

    pub fn is_empty(&self) -> bool {
        self.done.is_empty()
    }
}

#[cfg(test)]
mod tests;
