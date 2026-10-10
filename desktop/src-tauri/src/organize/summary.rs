//! 轮级总结的交付（Organize Summary Design v1 §3.3）：整理管道把总结落树
//! 投影（`MemoryTree.summaries`），**下一轮开场**（持会话锁、先于轮次组装）
//! 在此排空成会话日志的 `SessionEvent::ContextSummary`——位置恒在断层缝合
//! 处，从落位轮起随历史每轮在场，直到被下一次整理收编进材料滚动进新总结。
//!
//! 为什么不在管道里直接写日志：内存 `Session` 是唯一写真源，磁盘 append
//! 必须与内存索引对齐；管道不持会话锁，直接写盘会造成内存/磁盘事件位分叉。
//!
//! 已交付判定无跨文件标记：日志里 `(s.to, 下一条总结.to]` 区间内已有
//! ContextSummary 事件 = 已交付（自愈，崩溃不会双投）；`.to` 超出日志长度
//! = 底座事件已被截断重写，跳过不投（投影语义，设计稿 §5.5）。

use session_store::{Session, SessionEvent};

use super::Organizer;

impl Organizer {
    /// 排空未交付的轮级总结（agent_chat / 调度引擎轮开场调用，持锁）。
    /// 追加落日志 + 落盘镜像，返回本轮新交付的总结正文（调用方经 sink
    /// 推实时事件——与重放投影同形）。
    pub fn deliver_summaries(&self, session: &mut Session, session_id: &str) -> Vec<String> {
        let workspace = (self.workspace_of)(session_id).unwrap_or_default();
        let Some(tree) = self.tree_store.load(&workspace, session_id) else {
            return Vec::new();
        };
        if tree.summaries.is_empty() {
            return Vec::new();
        }
        // 事件位与已有交付位置先收集完（借用以不可变），追加阶段只碰
        // 可变借用——Session 借检不允许两段并存。
        let (events_len, cs_positions): (usize, Vec<usize>) = {
            let events = session.events();
            (
                events.len(),
                events
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| matches!(e, SessionEvent::ContextSummary { .. }))
                    .map(|(i, _)| i)
                    .collect(),
            )
        };
        let mut first_append: Option<usize> = None;
        let mut delivered = Vec::new();
        for (i, s) in tree.summaries.iter().enumerate() {
            // 底座事件位已不在日志（会话删除重建 / 截断重写）：跳过不投
            if s.to >= events_len {
                continue;
            }
            let upper = tree.summaries.get(i + 1).map_or(usize::MAX, |n| n.to);
            let already = cs_positions.iter().any(|&p| p > s.to && p <= upper);
            if already {
                continue;
            }
            let seq = session.append(SessionEvent::ContextSummary {
                text: s.text.clone(),
            });
            first_append.get_or_insert(seq);
            delivered.push(s.text.clone());
        }
        if let Some(from) = first_append {
            crate::session_log::append_slice(&self.sessions_dir, session_id, session, from);
        }
        delivered
    }
}
