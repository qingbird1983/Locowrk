//! 停止生成（v4.11 §3.39④，UI shell 文档）：会话级中止标记注册表。
//!
//! 每轮 `run_turn` 开跑前登记一个 `Arc<AtomicBool>`，收束后注销；`agent_stop`
//! 命令把标记置位。消费端两处——① LLM 流式循环（`StreamChunkSink::
//! should_stop`，delta 回调点逐段检查，命中即带部分正文收束）；② runner
//! step 循环顶（步间边界检查；已在执行的工具调用不可中断，如实边界）。
//!
//! 注册表是 std Mutex（存取不跨 await）；`unregister` 按指针等同比较——
//! 防止上一轮的注销误删下一轮刚登记的新标记（快速连发时的竞态）。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub struct StopFlags {
    flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl StopFlags {
    /// 轮次开跑前登记；返回标记句柄供收束时注销。
    pub fn register(self: &Arc<Self>, session_id: &str) -> Arc<AtomicBool> {
        let flag = Arc::new(AtomicBool::new(false));
        self.flags
            .lock()
            .expect("stop flags poisoned")
            .insert(session_id.to_string(), Arc::clone(&flag));
        flag
    }

    /// 轮次收束后注销：只摘自己那一代的标记（指针不同 = 已被换代，不动）。
    pub fn unregister(self: &Arc<Self>, session_id: &str, flag: &Arc<AtomicBool>) {
        let mut map = self.flags.lock().expect("stop flags poisoned");
        if map.get(session_id).is_some_and(|f| Arc::ptr_eq(f, flag)) {
            map.remove(session_id);
        }
    }

    /// `agent_stop`：置位（轮未在跑 = 空转，幂等）。
    pub fn request(&self, session_id: &str) {
        if let Some(f) = self
            .flags
            .lock()
            .expect("stop flags poisoned")
            .get(session_id)
        {
            f.store(true, Ordering::Relaxed);
        }
    }

    /// runner 步间边界与流式回调点的查询端。
    pub fn is_stopped(&self, session_id: &str) -> bool {
        self.flags
            .lock()
            .expect("stop flags poisoned")
            .get(session_id)
            .is_some_and(|f| f.load(Ordering::Relaxed))
    }
}

/// 轮次级 RAII 守卫：`guard()` 即登记，守卫丢弃（含 `?` 提前返回 / 恐慌
/// 展开）时自动注销——run_turn 的多条收束路径不用逐处补注销。
pub struct StopGuard {
    flags: Arc<StopFlags>,
    session_id: String,
    flag: Arc<AtomicBool>,
}

impl StopFlags {
    pub fn guard(self: &Arc<Self>, session_id: &str) -> StopGuard {
        let flag = self.register(session_id);
        StopGuard {
            flags: Arc::clone(self),
            session_id: session_id.to_string(),
            flag,
        }
    }
}

impl Drop for StopGuard {
    fn drop(&mut self) {
        self.flags.unregister(&self.session_id, &self.flag);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_request_check_unregister_roundtrip() {
        let flags = Arc::new(StopFlags::default());
        let flag = flags.register("s1");
        assert!(!flags.is_stopped("s1"));
        flags.request("s1");
        assert!(flags.is_stopped("s1"));
        assert!(flags.is_stopped("s1"), "置位后持续为真");
        flags.unregister("s1", &flag);
        assert!(!flags.is_stopped("s1"), "注销后查询回落");
    }

    #[test]
    fn unregister_ignores_newer_generation() {
        let flags = Arc::new(StopFlags::default());
        let old = flags.register("s1");
        flags.request("s1");
        // 上一代还没注销，新一代已开跑（register 顶掉同键；新标记 = false）
        let new = flags.register("s1");
        flags.unregister("s1", &old);
        // 旧代注销不得动新代条目（指针不同不摘）：map 里还是 new——
        // is_stopped 读到 new 的 false（若误摘了 new 读不到任何值；若
        // 误摘失败留下 old 会读到 true）。
        assert!(
            !flags.is_stopped("s1"),
            "旧代注销后 map 应仍是新代标记（值 false）"
        );
        // 新代标记仍可达可置位
        flags.request("s1");
        assert!(flags.is_stopped("s1"));
        flags.unregister("s1", &new);
        assert!(!flags.is_stopped("s1"));
    }

    #[test]
    fn request_unknown_session_is_noop() {
        let flags = Arc::new(StopFlags::default());
        flags.request("ghost");
        assert!(!flags.is_stopped("ghost"));
    }
}
