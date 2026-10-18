//! 活动调用登记表（specs/04 · C1，审计 H5 的第一刀）。
//!
//! **这一刀只解决一件事**：让"用户按了停止"这个事实能落到**正在跑的那次工具调用**手里。
//! 改前 `CallCtx` 的取消标志两端都没接（`plugin-api/src/tool.rs:191-218`：没有任何地方持有
//! 「本会话进行中调用」的登记表去逐个 `cancel()`），所以停止只落在 LLM 流式与步间边界，
//! 已经在跑的工具调用跑到底。
//!
//! 形状照 `desktop/src-tauri/src/stop.rs` 那份现成先例（std Mutex 登记表 + RAII 注销），
//! 两处差别是实测出来的，不是漂移：
//!
//! * **键里带 `call_id` 和一个登记序号**。`stop.rs` 的键只有 session_id，所以它必须
//!   `Arc::ptr_eq` 比指针防"上一轮的注销误删下一轮的登记"；这里的键每次都唯一（序号
//!   单调），换代在结构上不可能发生，于是那份比较**不需要**，也就没必要存在。
//! * **登记的是 `CallCtx` 克隆而不是 `Arc<AtomicBool>`**。`CallCtx` 是 `Clone` 且内部
//!   `cancelled: Arc<AtomicBool>` ⇒ 克隆那份与工具手里那份是**同一个**标志位，因此本模块
//!   不需要给 `plugin-api` 加"取出内部句柄"的新公开面——那是 AGENTS §5 的 [LOCK] 区，
//!   C1 一步都不该动它（真要把句柄交出去，是 C3 那张追加槽位的刀，要单独点头）。
//!
//! 只登记**带会话身份**的调用（`step_as` 那条通路）。不是省事：停止命令本身带
//! `session_id`（`commands.rs:137` 的 `agent_stop`），无会话归属的调用没有任何一条现成
//! 通路会去取消它，登记进来只会让表里多一堆没人按的条目。headless 的普通 `step`
//! 因此照旧不可取消——如实写在类型上，比造一个没人调的 `cancel_all()` 诚实。
//!
//! 诚实边界（免得被当成已做完）：这条信号现在只有**宿主内建的 Rust 工具**看得见
//! （它们直接实现 `Tool::call(&CallCtx, ..)`）。第三方插件看不见——`extern "C"` 的
//! `call` 七个参数不含 ctx、`HostVtable` 里没有轮询槽，那是 C3（动 ABI 面、要点头）；
//! `tool-shell` / `tool-fetch` 真去轮询是 C4；"被取消"在日志与界面上长什么样是 C5。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use plugin_api::CallCtx;

/// 键 = (会话, 调用 id, 第几次登记)。前两项供 `cancel_session` 匹配，第三项保证
/// 每一条登记都有自己的位置：重试复用同一 `call_id`（纪律 7 的幂等前提）时，
/// 前一次的注销不会抹掉后一次的登记。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Key {
    session: String,
    call: String,
    seq: u64,
}

/// 进行中的工具调用登记表。挂在 `Kernel` 上（`Kernel::active_calls()` 取得），
/// 子内核与父内核**共用同一张表**：同一进程里"什么在途"是一件事实，不该有两本账。
#[derive(Default)]
pub struct ActiveCalls {
    table: Mutex<HashMap<Key, CallCtx>>,
    /// 登记序号：`Ordering::Relaxed` 够用（它只要求唯一与单调，不参与同步语义）。
    next: AtomicU64,
}

/// 一次在途调用的守卫：丢弃（含 `?` 提前返回与恐慌展开）即注销。
///
/// 为什么是 RAII 而不是"调用点各自补一行注销"：那次调用上下有好几条早退路径
/// （中间件 Reject、审批拒绝、超时、恐慌），手写注销的失效方向正是"漏一条 =
/// 表里永留一条已死条目"，而残留的那条会让下一次按停止去 cancel 一个不存在的调用
/// ⇒ 置位条数这个读数开始说谎。先例：`stop.rs:63-84` 的 `StopGuard`。
pub struct ActiveGuard {
    table: Arc<ActiveCalls>,
    key: Key,
}

impl ActiveCalls {
    /// 登记一条在途调用，返回注销守卫。缺会话身份或缺 `call_id` ⇒ 不登记（返回 `None`）。
    pub fn track(self: &Arc<Self>, ctx: &CallCtx) -> Option<ActiveGuard> {
        let session = ctx.session_id.clone()?;
        let call = ctx.call_id.as_ref().map(|id| id.0.clone())?;
        let seq = self.next.fetch_add(1, Ordering::Relaxed);
        let key = Key { session, call, seq };
        self.table
            .lock()
            .expect("active calls table poisoned")
            .insert(key.clone(), ctx.clone());
        Some(ActiveGuard {
            table: Arc::clone(self),
            key,
        })
    }

    /// 取消该会话全部在途调用，返回**置位的条数**。
    ///
    /// 条数是给消费端用的读数，不是装饰：组装点把它打进停止日志（specs/04 C4 要的那条
    /// "在途调用被停止"的叙事），测试用它证注销真的发生（调用收束后再点一次必须得 0）。
    pub fn cancel_session(&self, session_id: &str) -> usize {
        let mut table = self.table.lock().expect("active calls table poisoned");
        let mut n = 0usize;
        for (k, ctx) in table.iter_mut() {
            if k.session == session_id {
                ctx.cancel();
                n += 1;
            }
        }
        n
    }

    /// 某会话当前在途几条（停止日志的"取消前"侧读数）。
    pub fn session_in_flight(&self, session_id: &str) -> usize {
        let table = self.table.lock().expect("active calls table poisoned");
        table.keys().filter(|k| k.session == session_id).count()
    }

    fn untrack(&self, key: &Key) {
        let mut table = self.table.lock().expect("active calls table poisoned");
        table.remove(key);
    }
}

impl Drop for ActiveGuard {
    fn drop(&mut self) {
        self.table.untrack(&self.key);
    }
}
