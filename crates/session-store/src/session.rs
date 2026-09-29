//! 会话：仅追加的事件日志 + 投影（DESIGN.md §4.2）。

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use plugin_api::CallId;

use crate::event::SessionEvent;
use crate::snapshot::SNAPSHOT_SCHEMA_VERSION;

/// 每 N 个事件落一个内部快照，`state_at` 从快照起步折算（§6.2）。
pub const SNAPSHOT_EVERY: usize = 64;

/// 投影给 LLM 的消息（会话日志的中立视图；模型适配层再映射各家协议）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallMsg {
    pub id: CallId,
    pub tool: String,
    pub args: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum Message {
    User {
        content: String,
    },
    /// AssistantMessage 事件只存 CallId；工具名与参数从 ToolCall 回填
    /// （真实工具协议要求 assistant 消息携带完整 tool_calls，§4.2）。
    Assistant {
        content: String,
        tool_calls: Vec<ToolCallMsg>,
    },
    ToolResult {
        call_id: CallId,
        tool: String,
        args: Value,
        output: Value,
        error: Option<String>,
    },
}

/// 会话状态 = fold(事件)。快照存它（schema_version 演进，§9.5）。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SessionState {
    pub messages: Vec<Message>,
}

impl SessionState {
    /// 单事件折叠。ToolCall 紧随 AssistantMessage：按 call_id 回填最近一条
    /// assistant 消息里的占位条目（先有空位、后有内容，两遍合一遍）。
    pub fn apply(&mut self, e: &SessionEvent) {
        match e {
            SessionEvent::UserMessage { content } => {
                self.messages.push(Message::User {
                    content: content.clone(),
                });
            }
            SessionEvent::AssistantMessage {
                content,
                tool_calls,
            } => {
                self.messages.push(Message::Assistant {
                    content: content.clone(),
                    tool_calls: tool_calls
                        .iter()
                        .map(|id| ToolCallMsg {
                            id: id.clone(),
                            tool: String::new(),
                            args: Value::Null,
                        })
                        .collect(),
                });
            }
            SessionEvent::ToolCall {
                call_id,
                tool,
                args,
                ..
            } => {
                if let Some(slot) = self.pending_slot(call_id) {
                    slot.tool = tool.clone();
                    slot.args = args.clone();
                }
            }
            SessionEvent::ToolResult {
                call_id,
                output,
                error,
            } => {
                let (tool, args) = self
                    .pending_slot(call_id)
                    .map(|s| (s.tool.clone(), s.args.clone()))
                    .unwrap_or_else(|| (String::new(), Value::Null));
                self.messages.push(Message::ToolResult {
                    call_id: call_id.clone(),
                    tool,
                    args,
                    output: output.clone(),
                    error: error.clone(),
                });
            }
            _ => {}
        }
    }

    /// 最近一条 assistant 消息中 call_id 对应的占位条目（§4.2 回填语义）。
    fn pending_slot(&mut self, call_id: &CallId) -> Option<&mut ToolCallMsg> {
        self.messages.iter_mut().rev().find_map(|m| match m {
            Message::Assistant { tool_calls, .. } => {
                tool_calls.iter_mut().find(|c| &c.id == call_id)
            }
            _ => None,
        })
    }
}

/// 会话（§4.2）：`log` 只追加；`cursor` 是可见前缀长度；回退只移动游标。
pub struct Session {
    // crate 内可见：snapshot.rs（恢复路径）需要物化重建；对外保持封装
    pub(crate) log: Vec<SessionEvent>,
    pub(crate) cursor: usize,
    pub(crate) snapshots: BTreeMap<usize, SessionState>,
    pub(crate) schema_version: u32,
}

impl Session {
    pub fn new() -> Self {
        Self {
            log: Vec::new(),
            cursor: 0,
            snapshots: BTreeMap::new(),
            schema_version: SNAPSHOT_SCHEMA_VERSION,
        }
    }

    /// 追加事件，返回序号。游标在末尾时随追加前移（正常流式写入）；
    /// 已回退（cursor < len）时游标不动——此时应 `fork` 出新会话继续，
    /// 原日志保持完整可审计（§4.2 / §6.2）。
    pub fn append(&mut self, e: SessionEvent) -> usize {
        if self.cursor == self.log.len() {
            self.cursor += 1;
        }
        self.log.push(e);
        let seq = self.log.len() - 1;
        if seq > 0 && seq % SNAPSHOT_EVERY == 0 {
            self.snapshots.insert(seq, self.state_at(seq));
        }
        seq
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn len(&self) -> usize {
        self.log.len()
    }

    pub fn is_empty(&self) -> bool {
        self.log.is_empty()
    }

    pub fn events(&self) -> &[SessionEvent] {
        &self.log
    }

    /// 纯函数投影：只看游标之前的事件（回退后不含已撤销的轮次，§4.2）。
    /// 两遍：先收集 ToolCall（call_id → tool / args），再拼装消息。
    pub fn derive_messages(&self) -> Vec<Message> {
        let visible = &self.log[..self.cursor.min(self.log.len())];

        let mut calls: HashMap<&CallId, (&str, &Value)> = HashMap::new();
        for e in visible {
            if let SessionEvent::ToolCall {
                call_id,
                tool,
                args,
                ..
            } = e
            {
                calls.insert(call_id, (tool.as_str(), args));
            }
        }

        visible
            .iter()
            .filter_map(|e| match e {
                SessionEvent::UserMessage { content } => Some(Message::User {
                    content: content.clone(),
                }),
                SessionEvent::AssistantMessage {
                    content,
                    tool_calls,
                } => Some(Message::Assistant {
                    content: content.clone(),
                    tool_calls: tool_calls
                        .iter()
                        .map(|id| {
                            let (tool, args) = backfill(calls.get(id));
                            ToolCallMsg {
                                id: id.clone(),
                                tool,
                                args,
                            }
                        })
                        .collect(),
                }),
                SessionEvent::ToolResult {
                    call_id,
                    output,
                    error,
                } => {
                    let (tool, args) = backfill(calls.get(call_id));
                    Some(Message::ToolResult {
                        call_id: call_id.clone(),
                        tool,
                        args,
                        output: output.clone(),
                        error: error.clone(),
                    })
                }
                _ => None,
            })
            .collect()
    }

    /// 状态 = fold(自最近快照起的事件)（§4.2 state_at）。
    pub fn state_at(&self, seq: usize) -> SessionState {
        let seq = seq.min(self.log.len());
        let (base_seq, mut st) = self
            .snapshots
            .range(..=seq)
            .next_back()
            .map(|(s, v)| (*s, v.clone()))
            .unwrap_or((0, SessionState::default()));
        for e in &self.log[base_seq..seq] {
            st.apply(e);
        }
        st
    }

    /// 回退：移动游标，不删事件（纪律：事件只追加不删除）。
    pub fn rewind_to(&mut self, seq: usize) {
        self.cursor = seq.min(self.log.len());
    }

    /// 回到某次工具调用之前（§6.2 rewind_before）。找不到返回 None。
    pub fn rewind_before(&mut self, call_id: &CallId) -> Option<usize> {
        let pos = self
            .log
            .iter()
            .position(|e| matches!(e, SessionEvent::ToolCall { call_id: c, .. } if c == call_id))?;
        self.rewind_to(pos);
        Some(pos)
    }

    /// 分叉：从已有日志创建新会话。只保留切断点之前的快照——超出的快照
    /// 指向已不存在的事件，会让 state_at 的日志切片越界 panic（§4.2）。
    pub fn fork(&self, at: usize) -> Session {
        let at = at.min(self.log.len());
        Session {
            log: self.log[..at].to_vec(),
            cursor: at,
            snapshots: self
                .snapshots
                .range(..=at)
                .map(|(s, v)| (*s, v.clone()))
                .collect(),
            schema_version: self.schema_version,
        }
    }
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

fn backfill(found: Option<&(&str, &Value)>) -> (String, Value) {
    found
        .map(|(t, a)| ((*t).to_string(), (*a).clone()))
        .unwrap_or_else(|| (String::new(), Value::Null))
}

// TurnId / TurnOutcome 目前由 kernel 的会话管理层（阶段四 SessionManager）使用，
// 这里 re-export 保持 §4.1 事件全集的类型完整。
#[allow(unused_imports)]
pub use crate::event::{TurnId, TurnOutcome};

#[cfg(test)]
mod tests;
