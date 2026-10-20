//! 墙上时钟读取唯一入口（《AUDIT-FIX-PLAN-v1》批 F·F1 收敛，审计 M-1 同族）。
//!
//! 收敛前 desktop 侧有 **9 份形状相同的本地实现**（`SystemTime::now() →
//! duration_since(UNIX_EPOCH) → map → unwrap_or(0)`）散在 `runner` / `organize` /
//! `organize/tree_tools` / `llm/diagnose` / `llm/catalog` / `media` / `memory` /
//! `memory_reconcile` / `scheduler/store`（其中 `organize` / `memory` /
//! `memory_reconcile` 三处已随 S4 刀③c 整线退役，清单是收敛时的历史读数），
//! 外加 5 处把同一段链**内联写开**的调用点
//! （`autodisable` / `commands/plugins` / `commands/sessions` / `usage` /
//! `notifications`）——彼此只差单位与返回类型。同语义实现分家正是「改一处漏一处」的
//! 产地（v2.133 的 ENV_LOCK 分裂、v2.135 的 gc 序错同源），所以现在收成**一把尺两个
//! 单位**：
//!
//! * [`now_ms`] —— 距 epoch 的毫秒（事件 `ts`、sqlite `run_at`、章节标记、id 后缀）
//! * [`now_secs`] —— 距 epoch 的秒（闲置巡检、使用统计）
//!
//! **需要别的整数形状就在调用点 `as`**（`llm/catalog.rs` 的 `loaded_ms: u64` 是既有
//! 序列化面，尺统一不等于换字段类型）；而 **纳秒造唯一 id** 的（`settings/llm.rs::
//! new_profile_id`、测试夹具 nonce）语义不是「读时间」而是「造身份」，**故意不进这里**
//! ——收进来只会让"读时钟"与"发号"看起来是一件事。`llm/retry.rs::jitter_random`
//! 取的是 `subsec_nanos` 当**熵源**（第三种语义），同理不收。单调钟 `Instant`（算时长差）
//! 不混；但**文件 mtime 换整数毫秒**走 [`to_epoch_ms`]——同一把尺，只是量的对象是
//! 别人的时间戳而不是"现在"。

use std::time::{SystemTime, UNIX_EPOCH};

/// 距 epoch 的原始毫秒；时钟异常（系统时间早于 epoch）按 0 处理——与收敛前
/// 九份实现的 `unwrap_or(0)` / `unwrap_or_default()` 同口径。
fn epoch_ms_of(now: SystemTime) -> u128 {
    now.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// 毫秒 → 秒的换算。与 `Duration::as_secs()` 等值（`as_millis` 已向下取整，
/// 再除 1000 与直接取秒一致）——等值判据钉在 `timeutil/tests.rs`，避免
/// "换了一条取整路径"悄悄漂一秒。
fn secs_of_millis(ms: u128) -> i64 {
    (ms / 1000) as i64
}

/// 距 epoch 的毫秒数（事件 `ts` / `run_at` / 章节标记 / 通知与页签 id 后缀）。
pub(crate) fn now_ms() -> i64 {
    to_epoch_ms(SystemTime::now())
}

/// 距 epoch 的秒数（闲置巡检 / 使用统计时间戳）。
pub(crate) fn now_secs() -> i64 {
    secs_of_millis(epoch_ms_of(SystemTime::now()))
}

/// 任意 `SystemTime` → 距 epoch 毫秒（文件 mtime 读数：按新旧裁决处拿它比时间）。
/// 与 [`now_ms`] 同一把尺，只是输入不是"现在"。
pub(crate) fn to_epoch_ms(t: SystemTime) -> i64 {
    epoch_ms_of(t) as i64
}

#[cfg(test)]
mod tests;
