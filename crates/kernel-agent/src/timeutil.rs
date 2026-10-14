//! 内核侧墙上时钟读取唯一入口（队列 Q12·第二刀 v2.205 收的同一族债）。
//!
//! 收敛前 `kernel-agent` 里有 **3 份形状相同的本地读数**：`kernel.rs` 的
//! `ContextCompacted` 那颗 `ts` 把 `SystemTime::now() → duration_since(UNIX_EPOCH) →
//! map → unwrap_or(0)` **整段内联写开**、`subagent/run.rs::now_ms` 与
//! `middleware/tool_exposure.rs::now_secs` 各持一份私有实现（只差单位与返回类型）。
//! 桌面端早在《AUDIT-FIX-PLAN-v1》批 F·F1 就把同一个问题收成了一把尺两个单位
//! （`desktop/src-tauri/src/timeutil.rs`），内核侧那半一直没动——本刀要给
//! `ApprovalDecided` 追加 `asked_at`/`decided_at` 两颗时刻，若照内联形状再写两处，
//! 这把尺就从 3 个出处涨到 5 个，正是 F1 当年点名的「改一处漏一处」产地。
//!
//! * [`now_ms`] —— 距 epoch 的毫秒（事件 `ts`、审批两时刻）
//! * [`now_secs`] —— 距 epoch 的秒（exposure 闲置巡检的时间衰减）
//!
//! **两个单位各自照改前的取整路径**（毫秒 `as_millis() as i64`、秒 `as_secs()`），
//! 本刀是**搬运不是重定义**：把 `now_secs` 改成「取毫秒再整除」会让单位换算路径换掉，
//! 那是另一件事（桌面尺的等值证明在 `timeutil/tests.rs`，此处等价性由既有调用点
//! 的行为不变来给，不由换写法来给）。
//!
//! ⚠ **与桌面端那把尺刻意不合体**：依赖方向是 `desktop/src-tauri → kernel-agent`，
//! 把内核尺做成 `pub` 让桌面端复用要牵动桌面 14 处调用点与 F1 的既有等价判据，
//! 收益只是"少一个文件"，代价是动一整批已收口的面 ⇒ 登记为边界不做（见 ROADMAP
//! v2.205 节边界段）。两把尺**语义同源**（同一个 epoch、同一个时钟不可用 = 0 的
//! 兜底口径），形状判据钉的是「每个 crate 内各只许一处定义」。

use std::time::{SystemTime, UNIX_EPOCH};

/// 距 epoch 的毫秒数（事件 `ts` / 审批两时刻）。时钟不可用（系统时间早于 epoch）
/// 按 0 处理——与收敛前三份本地实现的 `unwrap_or(0)` 同口径。
pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 距 epoch 的秒数（工具曝光度的闲置巡检：`tool_exposure` 的 EWMA 时间衰减）。
/// 单位与 [`now_ms`] 不同是**调用点既有的形状**，不是本批新造的第三种口径。
pub(crate) fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
