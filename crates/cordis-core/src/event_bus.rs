//! 事件总线（DESIGN.md §3.5）。
//!
//! 阶段一落地 `emit`；阶段四补齐另外三种分发语义：`parallel`（并发通知）、
//! `serial`（顺序可短路）、`waterfall`（顺序可改写/拒绝/重试）。waterfall
//! 决策点（agent/pre-step / agent/request / agent/request-error）复用同一
//! 注册表，是"连 Loop 都能热插拔"的工程实现（§3.5）。
//!
//! 纪律：先克隆监听器列表再 await，绝不持 DashMap 分片锁跨 await；
//! 绝不 fire-and-forget——`parallel` 也 join 全部任务再返回。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use dashmap::DashMap;
use serde_json::Value;

/// 事件监听器（emit / parallel 语义）。
#[async_trait::async_trait]
pub trait Listener: Send + Sync + 'static {
    async fn on_event(&self, key: &str, payload: &Value);
}

/// serial 监听器：顺序通知、可短路（审计链、安全过滤器，§3.5）。
/// 返回 `true` = 放行继续，`false` = 短路后续监听器。
#[async_trait::async_trait]
pub trait SerialListener: Send + Sync + 'static {
    async fn on_event(&self, key: &str, payload: &Value) -> bool;
}

/// waterfall 监听器：读借用 payload，返回决策（§3.5）。要改写就克隆
/// payload 返回 `Replace`——借用签名让 Continue / Reject / Retry 都不付克隆。
#[async_trait::async_trait]
pub trait WaterfallListener: Send + Sync + 'static {
    async fn handle(&self, key: &str, payload: &Value) -> WaterfallDecision;
}

/// waterfall 单环决策（§3.5）。
#[derive(Debug, Clone, PartialEq)]
pub enum WaterfallDecision {
    /// 放行，payload 原样传给下一环。
    Continue,
    /// 改写 payload 后继续传给下一环。
    Replace(Value),
    /// 拒绝并短路。
    Reject(RejectReason),
    /// 请求失败点专用：延迟后重试（受内核重试预算约束，§7.2）。
    Retry { delay: Duration },
}

/// 拒绝缘由：谁拒的、为什么。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RejectReason {
    /// 决策方标识（插件 id / 审批点名字）。
    pub by: String,
    pub reason: String,
}

/// waterfall 的返回值：最终 payload + 终态决策。
///
/// 设计伪代码只返回 `WaterfallDecision`，但内核需要改写后的 payload
/// 才能继续（§7.2 的 `parse_request`），v2.3 对齐为双字段结构。
#[derive(Debug, Clone, PartialEq)]
pub struct WaterfallOutcome {
    pub payload: Value,
    pub decision: WaterfallDecision,
}

/// serial 的结局。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialOutcome {
    /// 全部监听器放行。
    Completed,
    /// 第 `at` 个监听器短路（0 起）。
    Stopped { at: usize },
}

/// 监听器标识，`off` 用（三种注册表共用一个计数器）。
pub type ListenerId = u64;

/// 单个事件名的监听器列表（压平 type_complexity 的别名）。
type ListenerList<L> = Vec<(ListenerId, Arc<L>)>;

/// 四种分发语义的注册表（§3.5）。内部可变性：方法全部 `&self`。
#[derive(Default)]
pub struct EventBus {
    listeners: DashMap<String, ListenerList<dyn Listener>>,
    serials: DashMap<String, ListenerList<dyn SerialListener>>,
    waterfalls: DashMap<String, ListenerList<dyn WaterfallListener>>,
    next_id: AtomicU64,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_listener_id(&self) -> ListenerId {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// 注册 emit 监听器，返回 id 供 `off`。
    pub fn on(&self, key: impl Into<String>, listener: Arc<dyn Listener>) -> ListenerId {
        let id = self.next_listener_id();
        self.listeners
            .entry(key.into())
            .or_default()
            .push((id, listener));
        id
    }

    /// 注册 serial 监听器（审计链、安全过滤器）。
    pub fn on_serial(
        &self,
        key: impl Into<String>,
        listener: Arc<dyn SerialListener>,
    ) -> ListenerId {
        let id = self.next_listener_id();
        self.serials
            .entry(key.into())
            .or_default()
            .push((id, listener));
        id
    }

    /// 注册 waterfall 监听器（Agent 循环插桩、请求改写、审批）。
    pub fn on_waterfall(
        &self,
        key: impl Into<String>,
        listener: Arc<dyn WaterfallListener>,
    ) -> ListenerId {
        let id = self.next_listener_id();
        self.waterfalls
            .entry(key.into())
            .or_default()
            .push((id, listener));
        id
    }

    /// 注销监听器（三种注册表都查）。返回是否找到。
    pub fn off(&self, key: &str, id: ListenerId) -> bool {
        fn remove<L: ?Sized + 'static>(
            map: &DashMap<String, ListenerList<L>>,
            key: &str,
            id: ListenerId,
        ) -> bool {
            let Some(mut list) = map.get_mut(key) else {
                return false;
            };
            let before = list.len();
            list.retain(|(lid, _)| *lid != id);
            list.len() != before
        }
        remove(&self.listeners, key, id)
            || remove(&self.serials, key, id)
            || remove(&self.waterfalls, key, id)
    }

    /// 快照某事件名的监听器列表（克隆 Arc，绝不持分片锁跨 await）。
    fn snapshot<L: ?Sized + 'static>(
        map: &DashMap<String, ListenerList<L>>,
        key: &str,
    ) -> Vec<Arc<L>> {
        map.get(key)
            .map(|list| list.iter().map(|(_, l)| Arc::clone(l)).collect())
            .unwrap_or_default()
    }

    /// 异步通知（§3.5 表：日志、指标、遥测）。顺序 await 所有监听器，
    /// 保证确定性；绝不 spawn（无 fire-and-forget）。
    pub async fn emit(&self, key: &str, payload: &Value) {
        for listener in Self::snapshot(&self.listeners, key) {
            listener.on_event(key, payload).await;
        }
    }

    /// 并发通知所有监听者（§3.5 表：缓存刷新、索引更新）。join 全部任务
    /// 再返回；单个监听器 panic 只丢弃它自己的任务，不影响其他监听者。
    pub async fn parallel(&self, key: &str, payload: &Value) {
        let snapshot = Self::snapshot(&self.listeners, key);
        if snapshot.is_empty() {
            return;
        }
        let tasks: Vec<_> = snapshot
            .into_iter()
            .map(|listener| {
                let key = key.to_string();
                let payload = payload.clone();
                tokio::spawn(async move { listener.on_event(&key, &payload).await })
            })
            .collect();
        for task in tasks {
            let _ = task.await;
        }
    }

    /// 顺序通知、可短路（§3.5 表：审计链、安全过滤器）。
    pub async fn serial(&self, key: &str, payload: &Value) -> SerialOutcome {
        let snapshot = Self::snapshot(&self.serials, key);
        for (at, listener) in snapshot.into_iter().enumerate() {
            if !listener.on_event(key, payload).await {
                return SerialOutcome::Stopped { at };
            }
        }
        SerialOutcome::Completed
    }

    /// 顺序经过决策环（§3.5 表：Agent 循环插桩、请求改写、审批）：
    /// Continue 传原 payload，Replace 传改写后的，Reject / Retry 当场
    /// 短路并作为终态决策返回。无监听器 = 原样放行。
    pub async fn waterfall(&self, key: &str, mut payload: Value) -> WaterfallOutcome {
        for listener in Self::snapshot(&self.waterfalls, key) {
            match listener.handle(key, &payload).await {
                WaterfallDecision::Continue => {}
                WaterfallDecision::Replace(v) => payload = v,
                WaterfallDecision::Reject(reason) => {
                    return WaterfallOutcome {
                        payload,
                        decision: WaterfallDecision::Reject(reason),
                    };
                }
                WaterfallDecision::Retry { delay } => {
                    return WaterfallOutcome {
                        payload,
                        decision: WaterfallDecision::Retry { delay },
                    };
                }
            }
        }
        WaterfallOutcome {
            payload,
            decision: WaterfallDecision::Continue,
        }
    }
}

#[cfg(test)]
mod tests;
