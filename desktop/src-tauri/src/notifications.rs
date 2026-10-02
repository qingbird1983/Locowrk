//! 应用内通知记录（UI shell 文档 §3.13，§8.2「通知记录（应用内持久）」）：
//! desktop 自持一份「用户可能错过的事」——与 ⑭ 系统通知兜底并存，与 §3.3
//! 会话内系统消息不同层（那个跟会话走、重放可见，这个跟应用走）。存储 =
//! `<数据根>/notifications.json`（cap 100 FIFO，坏文件当空表）。入账 =
//! [`record_event`]（TauriSink::emit 调用）对五类事件的统一提取：定时触发 /
//! 子 Agent 收束成败 / 压缩完成 / 审批超时自动拒绝——纯提取在
//! [`draft_from_event`]（call_id→tool 配对表由调用方持有），ApprovalHub 与
//! 调度引擎零改动。入账后 emit `lw://notification`（载荷 = 新记录）。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use crate::approval::TIMEOUT_REASON_PREFIX;
use crate::events::AgentEvent;

/// 通知上限（FIFO 溢出删最旧）：浮层一屏的数据量级，100 条足够追溯。
const CAP: usize = 100;

/// call_id → 工具名配对表上限：条目只在 ToolCall→ToolResult 之间活一瞬；
/// 满则清空（丢的只是超长轮里子 Agent 收束通知这一条，不影响其余）。
const PAIR_CAP: usize = 64;

/// 应用内通知推送通道（前端 notifications store 订阅）。
pub const CHANNEL: &str = "lw://notification";

/// 通知类别。存 **kind + 参数**，标题模板由前端 i18n 渲染（§3.3 同纪律：
/// 文案是 UI 责任，后端不存文案）。
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    /// 定时触发（⑭）：label = 调度名。
    #[default]
    ScheduleFired,
    /// 子 Agent 收束（成功）。
    SubagentDone,
    /// 子 Agent 收束（失败/预算耗尽/超时）：label = 错误摘要。
    SubagentFailed,
    /// 上下文压缩完成（⑨）：label = 省下的 token 数（十进制字符串）。
    ContextCompacted,
    /// 审批超时自动拒绝（v2.48 守夜，fail-closed）。
    ApprovalTimeout,
}

/// 单条通知（wire 形态 camelCase，前端镜像在 `desktop/src/types.ts`）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NotificationItem {
    pub id: String,
    pub kind: NotificationKind,
    pub session_id: Option<String>,
    /// 类别语义的参数（见 [`NotificationKind`]），是数据值不是文案。
    pub label: Option<String>,
    /// Unix 秒（与会话列表 mtime 口径一致，前端过 §3.5 相对时间）。
    pub created_at: i64,
    pub read: bool,
}

/// 提取产物：入账三要素（[`draft_from_event`] 的返回）。
pub struct NotificationDraft {
    pub kind: NotificationKind,
    pub session_id: Option<String>,
    pub label: Option<String>,
}

/// 通知存储：内存表 + JSON 文件全量重写（条目量级小，追加即重写可接受；
/// 打不开文件只丢持久化，不拦入账）。
pub struct NotificationStore {
    path: PathBuf,
    /// 时间升序，最新在尾；溢出删头部。
    items: Mutex<Vec<NotificationItem>>,
}

static SEQ: AtomicU64 = AtomicU64::new(0);

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

impl NotificationStore {
    /// 打开（启动时一次）：坏 / 缺文件当空表，不拦启动。
    pub fn open(path: PathBuf) -> Self {
        let items = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Vec<NotificationItem>>(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            items: Mutex::new(items),
        }
    }

    /// 入账：置 id / 时间 / 未读，溢出删最旧，同步重写文件，返回新条目
    /// （调用方据此 emit 推送）。
    pub fn add(
        &self,
        kind: NotificationKind,
        session_id: Option<String>,
        label: Option<String>,
    ) -> NotificationItem {
        let item = NotificationItem {
            id: format!(
                "n-{}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ),
            kind,
            session_id,
            label,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0),
            read: false,
        };
        let mut items = self.items.lock().expect("notifications poisoned");
        items.push(item.clone());
        if items.len() > CAP {
            let overflow = items.len() - CAP;
            items.drain(0..overflow);
        }
        drop(items);
        self.persist();
        item
    }

    /// 全量（新→旧，浮层列表序）。未读数由前端按列表计（未读 = 状态，
    /// 列表即全量数据源，无需第二条命令）。
    pub fn list(&self) -> Vec<NotificationItem> {
        let items = self.items.lock().expect("notifications poisoned");
        items.iter().rev().cloned().collect()
    }

    /// 全部已读：返回置位数。
    pub fn mark_all_read(&self) -> usize {
        let mut items = self.items.lock().expect("notifications poisoned");
        let mut n = 0;
        for i in items.iter_mut() {
            if !i.read {
                i.read = true;
                n += 1;
            }
        }
        drop(items);
        if n > 0 {
            self.persist();
        }
        n
    }

    pub fn mark_read(&self, id: &str) -> bool {
        let hit = {
            let mut items = self.items.lock().expect("notifications poisoned");
            let hit = items.iter_mut().find(|i| i.id == id).map(|i| {
                i.read = true;
            });
            hit.is_some()
        };
        if hit {
            self.persist();
        }
        hit
    }

    pub fn clear(&self) {
        self.items.lock().expect("notifications poisoned").clear();
        self.persist();
    }

    fn persist(&self) {
        let bytes = serde_json::to_vec(&*self.items.lock().expect("notifications poisoned"))
            .expect("notifications serialize");
        if let Err(e) = std::fs::write(&self.path, bytes) {
            tracing::warn!("notifications 落盘失败: {e}");
        }
    }
}

/// ToolCall→ToolResult 配对表（call_id → 工具名）。模块级单例：进程内
/// 唯一事件出口（TauriSink）串行调用，Mutex 足够。
fn pairing() -> &'static Mutex<HashMap<String, String>> {
    static PAIR: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    PAIR.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 从 wire 事件提取通知草稿；非入账事件返回 None。`pending` = 配对表
/// （ToolCall 记、ToolResult 用），抽出来便于纯函数测试。
pub fn draft_from_event(
    event: &AgentEvent,
    pending: &mut HashMap<String, String>,
) -> Option<NotificationDraft> {
    match event {
        AgentEvent::ScheduleFired {
            session_id, title, ..
        } => Some(NotificationDraft {
            kind: NotificationKind::ScheduleFired,
            session_id: Some(session_id.clone()),
            label: Some(clip(title, 80)),
        }),
        AgentEvent::ToolCall { call_id, tool, .. } => {
            if pending.len() >= PAIR_CAP {
                pending.clear();
            }
            pending.insert(call_id.clone(), tool.clone());
            None
        }
        AgentEvent::ToolResult {
            session_id,
            call_id,
            ok,
            error,
            ..
        } => {
            let tool = pending.remove(call_id)?;
            if tool != "subagent_run" {
                return None;
            }
            Some(NotificationDraft {
                kind: if *ok {
                    NotificationKind::SubagentDone
                } else {
                    NotificationKind::SubagentFailed
                },
                session_id: Some(session_id.clone()),
                label: error.as_deref().map(|e| clip(e, 120)),
            })
        }
        AgentEvent::ContextCompacted {
            session_id,
            from_tokens,
            to_tokens,
            ..
        } => Some(NotificationDraft {
            kind: NotificationKind::ContextCompacted,
            session_id: Some(session_id.clone()),
            label: Some(from_tokens.saturating_sub(*to_tokens).to_string()),
        }),
        // 审批超时自动拒绝（守夜 fail-closed）：只认共享前缀常量，用户
        // 手动拒绝不入账（那是用户自己的操作，不是错过的事）。
        AgentEvent::ApprovalResolved {
            session_id,
            allowed: false,
            reason: Some(reason),
            ..
        } if reason.starts_with(TIMEOUT_REASON_PREFIX) => Some(NotificationDraft {
            kind: NotificationKind::ApprovalTimeout,
            session_id: Some(session_id.clone()),
            label: None,
        }),
        _ => None,
    }
}

/// TauriSink::emit 的入账钩子：提取 → 落 store → 推通道。store 尚未
/// manage（极早启动窗口）则静默跳过。
pub fn record_event(app: &AppHandle, event: &AgentEvent) {
    let Some(draft) = draft_from_event(event, &mut pairing().lock().expect("pairing poisoned"))
    else {
        return;
    };
    let Some(store) = app.try_state::<std::sync::Arc<NotificationStore>>() else {
        return;
    };
    let item = store.add(draft.kind, draft.session_id, draft.label);
    if let Err(e) = app.emit(CHANNEL, &item) {
        tracing::warn!("emit {CHANNEL} failed: {e}");
    }
}

#[cfg(test)]
#[path = "notifications/tests.rs"]
mod tests;
