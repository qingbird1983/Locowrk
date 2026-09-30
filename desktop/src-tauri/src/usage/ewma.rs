//! ⑫ EWMA 积累与打分桥（exposure 文档 §5.1/§5.3）：衰减公式 +
//! kernel-agent `GlobalEwma` 契约的宿主实现（"内核定契约、组装点实现"，
//! 与 ApprovalResolver / MemorySource 同模式）。

use std::collections::HashMap;
use std::sync::Arc;

use super::UsageRecorder;

/// EWMA 更新（exposure 文档 §5.1）：`old × exp(-days/7) + 本次调用数`，
/// 半衰期约 5 天——最近用过的权重高，三个月前的百次调用不压榜首。
/// `now` = 本窗口最新调用时刻，`last` = 上次落库的 last_used_at；
/// `calls` = 本窗口调用数（5s 批量刷盘一次结清，同窗口视作同时刻）。
pub(crate) fn update_ewma(old: f64, now: i64, last: i64, calls: u64) -> f64 {
    let days = ((now - last).max(0) as f64 / 86_400.0).min(f64::MAX / 2.0);
    old * (-days / 7.0).exp() + calls as f64
}

/// ToolExposure 打分读数：ewma_scores() 透传 usage.sqlite3（降级 = 空表，
/// 没有全局热度只是退回"会话内优先"，截断照常工作）。
pub struct UsageEwma {
    usage: Arc<UsageRecorder>,
}

impl UsageEwma {
    pub fn new(usage: Arc<UsageRecorder>) -> Arc<Self> {
        Arc::new(Self { usage })
    }
}

impl kernel_agent::GlobalEwma for UsageEwma {
    fn ewma_scores(&self) -> HashMap<String, f64> {
        self.usage.ewma_scores()
    }
}
