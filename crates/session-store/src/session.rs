//! 会话：仅追加的事件日志 + 投影（DESIGN.md §4.2）。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use plugin_api::CallId;

use crate::content::ContentPart;
use crate::event::SessionEvent;
use crate::snapshot::SNAPSHOT_SCHEMA_VERSION;

mod project;
use project::{compact_messages, project_messages};

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
    ///
    /// **命中即停**（审计 M-24b）：只检查反向遇到的**第一条** assistant，
    /// 它里面没有该 call_id 就返回 `None`，不再一路扫到日志头。旧写法
    /// （`find_map` 跳过所有非匹配 assistant）在 call_id 无占位时付
    /// O(整段历史)——而补偿路径每个 undo 都用从未被任何 assistant 声明过的
    /// `{id}-undo` 新 id（`kernel-agent/src/compensation.rs:101`），长会话里
    /// 这类事件越多，`apply`/`state_at`/`load_session` 越接近 O(n²)。
    ///
    /// 两种写法在**全部可达输入**上逐事件等价，依据是内核的追加次序恒为
    /// 一条 assistant 之后紧跟它自己的 tool_call/tool_result、下一条
    /// assistant 只出现在下一 step（`kernel-agent/src/kernel.rs:257/:310/:335`，
    /// 工具**串行** await，无并行执行）；崩溃恢复的悬空调用合成只发生在投影层、
    /// **不回填日志**（`uncertain.rs:15-16`），回放与快照恢复都按日志原序喂
    /// `apply`（`persist.rs:78-96`、`snapshot.rs:109-121`）。唯一会让两者分岔的
    /// 形状是「晚到的 ToolResult 跨过一条更新的 assistant 去回填更早的占位」，
    /// 该形状现网不可达；`session/tests.rs` 的 `pending_slot_stops_at_nearest_assistant`
    /// 把这一点钉住，将来若真引入乱序追加，那条测试会先说话。
    fn pending_slot(&mut self, call_id: &CallId) -> Option<&mut ToolCallMsg> {
        let tool_calls = self.messages.iter_mut().rev().find_map(|m| match m {
            Message::Assistant { tool_calls, .. } => Some(tool_calls),
            _ => None,
        })?;
        tool_calls.iter_mut().find(|c| &c.id == call_id)
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

#[cfg(test)]
mod tests;
