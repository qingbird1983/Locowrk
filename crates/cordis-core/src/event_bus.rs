//! 事件总线（DESIGN.md §3.5）。
//!
//! 阶段一落地 `emit` 语义（异步通知，不等待返回值）；`parallel` / `serial` /
//! `waterfall` 三种分发语义与中间件套件在阶段四补齐。waterfall 决策点
//! （agent/pre-step / agent/request / agent/request-error）复用同一注册表。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use dashmap::DashMap;
use serde_json::Value;

/// 事件监听器。waterfall（阶段四）在此基础上扩展决策返回值。
#[async_trait::async_trait]
pub trait Listener: Send + Sync + 'static {
    async fn on_event(&self, key: &str, payload: &Value);
}

/// 监听器标识，`off` 用。
pub type ListenerId = u64;

/// 单个事件名的监听器列表（压平 type_complexity 的别名）。
type ListenerList = Vec<(ListenerId, Arc<dyn Listener>)>;

/// 四种分发语义的注册表（§3.5）。内部可变性：方法全部 `&self`。
#[derive(Default)]
pub struct EventBus {
    listeners: DashMap<String, ListenerList>,
    next_id: AtomicU64,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册监听器，返回 id 供 `off`。
    pub fn on(&self, key: impl Into<String>, listener: Arc<dyn Listener>) -> ListenerId {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.listeners
            .entry(key.into())
            .or_default()
            .push((id, listener));
        id
    }

    /// 注销监听器。返回是否找到。
    pub fn off(&self, key: &str, id: ListenerId) -> bool {
        let Some(mut list) = self.listeners.get_mut(key) else {
            return false;
        };
        let before = list.len();
        list.retain(|(lid, _)| *lid != id);
        list.len() != before
    }

    /// 异步通知（§3.5 表：日志、指标、遥测）。顺序 await 所有监听器，
    /// 保证确定性；绝不 spawn（无 fire-and-forget）。
    /// 纪律：先克隆监听器列表再 await，绝不持 DashMap 分片锁跨 await。
    pub async fn emit(&self, key: &str, payload: &Value) {
        let snapshot: Vec<_> = self
            .listeners
            .get(key)
            .map(|list| list.iter().map(|(_, l)| Arc::clone(l)).collect())
            .unwrap_or_default();
        for listener in snapshot {
            listener.on_event(key, payload).await;
        }
    }
}

#[cfg(test)]
mod tests;
