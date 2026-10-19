//! 队列 Q10（L4 / ML 稿 §3.1 原则 4 + §5.1）：**注入观测面板的读端**。
//!
//! 分工：`recall_log` 只管"库里那一串字节"（它连 JSON 都不解析），本文件只管
//! "把那串字节讲成人能看懂的一屏"——取数、派生、拼 IPC 载荷。
//! 生产读取点只有一个：`commands/memory.rs::memory_inject_view`（薄适配层，
//! `check-commands` 三向闭合盯着），前端 `components/memory/InjectSection.vue` 消费。
//!
//! ⚠ **S3 刀③b 摘掉的是"最近一次注入的四信号解释"整块**（`explain`：`InjectExplain` /
//! `ExplainRow` / 坏字节降级 / 往回扫 50 行找有回执的那一条）。它的数据源是
//! `memory_inject_log.breakdown` 那一列——"那一次加权排序算出来的分数回执"，生产者是
//! `memory_learn/explain.rs::breakdown_json`，随 `memory_learn/` 整族与设置页那格
//! `learnEnabled` 同刀退役（裁定 = `specs/10-context-injection-s3` 开工三问②）。
//! 为什么摘**整块**而不是"让它显示空态"：一个恒 `present = false` 的面板区块就是审计报告
//! H2 判过的那个形状（存在但从未接线），而那三条状态用例（有回执 / 没回执 / 坏字节）
//! 会集体退化成断言同一个常量——证明力归零，代码还在。
//!
//! **剩下的这一屏不依赖任何分数**：L0 那张表的三个派生读数（注入次数 / 空块率 /
//! 注入产出率）由 `RecallLog::stats()` 现算，检索回执照常进分母。将来若要恢复
//! "这一轮到底注入了哪几条"的逐行视图，正解是给**索引段**埋点（specs/10 登记的欠账 ⓠ），
//! 不是把 `breakdown` 那一列捡回来。

use serde::Serialize;

use crate::recall_log::InjectStats;

/// L0 那张表的派生读数（面板顶部一行）。**Option 的两个比率不许在前端折算成 0**——
/// "没注入过"与"注入了但都空"是两个结论（同 `InjectStats` 的口径）。
#[derive(Debug, Clone, Serialize)]
pub struct InjectOverview {
    pub injections: usize,
    pub injected_keys: usize,
    pub yielded_keys: usize,
    pub store_empty_ratio: Option<f64>,
    pub injection_yield: Option<f64>,
}

/// 管理页只读区块的整份载荷。
#[derive(Debug, Clone, Serialize)]
pub struct InjectView {
    pub overview: InjectOverview,
}

/// 面板数据源：读观测表 + 派生指标。
pub(crate) fn view(log: &crate::recall_log::RecallLog) -> Result<InjectView, String> {
    Ok(InjectView {
        overview: overview_of(&log.stats()?),
    })
}

fn overview_of(stats: &InjectStats) -> InjectOverview {
    InjectOverview {
        injections: stats.injections,
        injected_keys: stats.injected_keys,
        yielded_keys: stats.yielded_keys,
        store_empty_ratio: stats.store_empty_ratio,
        injection_yield: stats.injection_yield,
    }
}

#[cfg(test)]
mod tests;
