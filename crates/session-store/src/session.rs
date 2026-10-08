//! 会话：仅追加的事件日志 + 投影（DESIGN.md §4.2）。

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use plugin_api::CallId;

use crate::content::ContentPart;
use crate::event::{SessionEvent, UncertainDecision};
use crate::snapshot::SNAPSHOT_SCHEMA_VERSION;
use crate::uncertain::closure;

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
    /// `parts`（批 D，serde default）= 附件部件（图片/PDF，`content` 为正
    /// 文）；空 = 纯文本消息（旧 jsonl/旧快照缺字段读入即此形态，零迁移；
    /// 空 parts 序列化时跳过——纯文本消息落盘行与旧格式同形）。
    User {
        content: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        parts: Vec<ContentPart>,
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

impl Message {
    /// 纯文本用户消息（批 D 起绝大多数构造点的简写；附件走
    /// [`Message::user_with_parts`]）。
    pub fn user(content: impl Into<String>) -> Self {
        Message::User {
            content: content.into(),
            parts: Vec::new(),
        }
    }

    /// 带附件的用户消息（批 D）。
    pub fn user_with_parts(content: impl Into<String>, parts: Vec<ContentPart>) -> Self {
        Message::User {
            content: content.into(),
            parts,
        }
    }
}

impl SessionState {
    /// 单事件折叠。ToolCall 紧随 AssistantMessage：按 call_id 回填最近一条
    /// assistant 消息里的占位条目（先有空位、后有内容，两遍合一遍）。
    pub fn apply(&mut self, e: &SessionEvent) {
        match e {
            SessionEvent::UserMessage { content, parts } => {
                self.messages.push(Message::User {
                    content: content.clone(),
                    parts: parts.clone(),
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
            SessionEvent::ContextCompacted {
                kept_messages,
                summary,
                ..
            } => compact_messages(&mut self.messages, *kept_messages, summary),
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

/// 压缩折叠（Context and Memory.md §3.7）：留尾部 `kept` 条，其余替换为
/// 一条摘要 User 消息（滑动窗口摘要为空串 → 不留摘要，纯丢早期）。
/// `kept >= len`（无头部可压）时空操作。apply（快照折叠）与
/// derive_messages 共用。
/// 事件切片 → 消息投影（`derive_messages` / `derive_messages_from` 共用
/// 单点实现，两遍：ToolCall 索引 → 拼装；M4 自 derive_messages 抽出）。
/// G2 悬空闭合：assistant 声明了 call_id 而切片内没有 ToolResult 事件
/// （进程在执行中退出的悬空调用，可靠性稿 §5）→ 紧贴 assistant 之后
/// 合成一条诚实闭合的 ToolResult 消息（未裁决 =「效果未知」；有裁决 =
/// 按裁决闭合）——不给合成，下一轮请求带悬空 tool_calls 会被供应商
/// 整单拒收。快照折叠（`apply`）不合成：快照只服务存档面，请求装配
/// 唯一真源是本函数。
fn project_messages(events: &[SessionEvent]) -> Vec<Message> {
    let mut calls: HashMap<&CallId, (&str, &Value)> = HashMap::new();
    let mut closed: HashSet<&CallId> = HashSet::new();
    let mut decisions: HashMap<&CallId, (UncertainDecision, &str)> = HashMap::new();
    for e in events {
        match e {
            SessionEvent::ToolCall {
                call_id,
                tool,
                args,
                ..
            } => {
                calls.insert(call_id, (tool.as_str(), args));
            }
            SessionEvent::ToolResult { call_id, .. } => {
                closed.insert(call_id);
            }
            SessionEvent::ToolExecutionUncertain {
                call_id,
                decision,
                note,
                ..
            } => {
                decisions.insert(call_id, (*decision, note.as_str()));
            }
            _ => {}
        }
    }

    let mut messages: Vec<Message> = Vec::new();
    for e in events {
        match e {
            SessionEvent::UserMessage { content, parts } => messages.push(Message::User {
                content: content.clone(),
                parts: parts.clone(),
            }),
            SessionEvent::AssistantMessage {
                content,
                tool_calls,
            } => {
                messages.push(Message::Assistant {
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
                });
                // 悬空闭合（G2）：声明位没有 ToolResult 事件 = 按裁决或
                // 「效果未知」合成。位置紧贴 assistant——tool_calls 与
                // tool 消息之间的先后是供应商硬约束。
                for id in tool_calls {
                    if !closed.contains(id) {
                        let (tool, args) = backfill(calls.get(id));
                        let (output, error) = closure(decisions.get(id).map(|(d, n)| (*d, *n)));
                        messages.push(Message::ToolResult {
                            call_id: id.clone(),
                            tool,
                            args,
                            output,
                            error,
                        });
                    }
                }
            }
            SessionEvent::ToolResult {
                call_id,
                output,
                error,
            } => {
                let (tool, args) = backfill(calls.get(call_id));
                messages.push(Message::ToolResult {
                    call_id: call_id.clone(),
                    tool,
                    args,
                    output: output.clone(),
                    error: error.clone(),
                });
            }
            SessionEvent::ContextCompacted {
                kept_messages,
                summary,
                ..
            } => compact_messages(&mut messages, *kept_messages, summary),
            _ => {}
        }
    }
    messages
}

fn compact_messages(messages: &mut Vec<Message>, kept: usize, summary: &str) {
    let keep = kept.min(messages.len());
    let head_len = messages.len() - keep;
    let tail = messages.split_off(head_len);
    let head = std::mem::take(messages);
    if !summary.is_empty() && !head.is_empty() {
        messages.push(Message::user(format!(
            "[earlier conversation summarized]\n{summary}"
        )));
    }
    messages.extend(tail);
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
    /// 两遍：先收集 ToolCall（call_id → tool / args），再单遍累计拼装；
    /// 途中遇到 ContextCompacted 按压缩折叠改写已累计消息（与 apply 的
    /// 快照折叠同一语义，`compact_messages`）。
    pub fn derive_messages(&self) -> Vec<Message> {
        let end = self.cursor.min(self.log.len());
        project_messages(&self.log[..end])
    }

    /// 树会话尾巴投影（M4 记忆树切流）：从事件下标 `from` 起跑同一投影
    /// （`[from..cursor)`）——`[0..from)` 已归捆进记忆树目录，请求只带
    /// 未整理尾巴。切在半轮上由调用方兜底前进到 User 边界（内核组装侧
    /// 职责）；切片内恰有 ContextCompacted 时 clamp 已由
    /// [`compact_messages`] 保证（最多把切片折叠到摘要为止，不越界）。
    pub fn derive_messages_from(&self, from: usize) -> Vec<Message> {
        let end = self.cursor.min(self.log.len());
        let from = from.min(end);
        project_messages(&self.log[from..end])
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
