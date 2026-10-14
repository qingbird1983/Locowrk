//! 队列 Q10（L4 / ML 稿 §3.1 原则 4 + §5.1）：**注入解释面板的读端**。
//!
//! 分工：`recall_log` 只管"库里那一串字节"（它连 JSON 都不解析），本文件只管
//! "把那串字节讲成人能看懂的一屏"——找行、解析、降级、拼 IPC 载荷。
//! 生产读取点只有一个：`commands/memory.rs::memory_inject_view`（薄适配层，
//! `check-commands` 三向闭合盯着），前端 `components/memory/InjectSection.vue` 消费。
//!
//! ⚠ **两条口径**：
//! - **找"最近一次真有回执的注入"，不是"最近一次注入"**。学习档关着的会话照常埋点
//!   但 `breakdown` 是 NULL（三档语义在 `recall_log/tests/breakdown.rs`），把它当
//!   "没有解释"是对的，把它当"最新一次注入没什么可说"就挡住了用户想看的那一条。
//! - **解析失败不许上抛**：库里那一串读不成 JSON（手改表 / 更早版本留下的坏字节）时
//!   返回 `unreadable = true` 让面板如实说"这一条读不出来"。整条通路报错 = 观测面
//!   把自己的数据源判死，用户连"有多少次注入、空块率"都看不到了。

use serde::Serialize;

use crate::recall_log::{InjectStats, MODE_INJECT};

/// 往回扫多少行找那一次可解释的注入。50 = 面板要的是"最近那一次"，而这条窗口
/// 同时挡住"把整张观测表读进 IPC 载荷"（表按会话一行，50 行就是 50 个会话键位）。
const SCAN_ROWS: i64 = 50;

/// 一行的解释：键名 + 四档占比 + 总分。顺序与 `memory_learn` 的权重序一致
/// （relevance, strength, freshness, recency），前端按位取标签、不按名字。
#[derive(Debug, Clone, Serialize)]
pub struct ExplainRow {
    pub key: String,
    pub share: [i64; 4],
    pub total: i64,
}

/// 最近一次可解释注入的回执。
#[derive(Debug, Clone, Serialize)]
pub struct InjectExplain {
    /// 库里找到带回执的行了吗。`false` = 面板走空态（学习档关着 / 一次注入都没发生）。
    pub present: bool,
    /// 有回执但**读不成 JSON**（坏字节）：面板要说"读不出来"而不是"没有"。
    pub unreadable: bool,
    pub ts: i64,
    pub session_key: String,
    pub scope_id: String,
    pub weights: [u16; 4],
    pub rows: Vec<ExplainRow>,
}

impl InjectExplain {
    fn absent() -> Self {
        InjectExplain {
            present: false,
            unreadable: false,
            ts: 0,
            session_key: String::new(),
            scope_id: String::new(),
            weights: [0; 4],
            rows: Vec::new(),
        }
    }
}

/// L0 那张表的三个派生读数（面板顶部一行）。**Option 的两个比率不许在
/// 前端折算成 0**——"没注入过"与"注入了但都空"是两个结论（同 `InjectStats` 的口径）。
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
    pub explain: InjectExplain,
    pub overview: InjectOverview,
}

/// 面板数据源：读观测表 + 派生指标，解析最近一次可解释的注入。
pub(crate) fn view(log: &crate::recall_log::RecallLog) -> Result<InjectView, String> {
    let rows = log.recent(SCAN_ROWS)?;
    let stats = log.stats()?;
    let explain = rows
        .iter()
        .find(|r| r.mode == MODE_INJECT && r.breakdown.is_some())
        .map(explain_of)
        .unwrap_or_else(InjectExplain::absent);
    Ok(InjectView {
        explain,
        overview: overview_of(&stats),
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

/// 四个数的数组读法：缺位补 0、多位截断。库里那一串是**上一版**写的，读端不能
/// 假设它与自己同版本——假设了就是一个坏字节把面板打崩。
fn arr4(v: &serde_json::Value) -> [i64; 4] {
    let mut out = [0i64; 4];
    if let Some(list) = v.as_array() {
        for (i, item) in list.iter().take(4).enumerate() {
            out[i] = item.as_i64().unwrap_or(0);
        }
    }
    out
}

fn arr4u16(v: &serde_json::Value) -> [u16; 4] {
    let mut out = [0u16; 4];
    for (i, value) in arr4(v).into_iter().enumerate() {
        out[i] = value.clamp(0, i64::from(u16::MAX)) as u16;
    }
    out
}

/// 单行解析：坏 JSON ⇒ `unreadable`，好 JSON 但形状不对 ⇒ 能读多少算多少。
fn explain_of(row: &crate::recall_log::RecallRow) -> InjectExplain {
    let mut out = InjectExplain {
        present: true,
        unreadable: false,
        ts: 0,
        session_key: row.session_key.clone(),
        scope_id: row.scope_id.clone(),
        weights: [0; 4],
        rows: Vec::new(),
    };
    out.ts = row.ts;
    let Some(json) = row.breakdown.as_deref() else {
        out.present = false;
        return out;
    };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else {
        out.unreadable = true;
        return out;
    };
    out.weights = arr4u16(&v["weights"]);
    // `rows` 不是数组 = 这一串根本不是本面板认得的回执形状 ⇒ 如实报"读不出来"。
    // ⚠ **空数组不是坏数据**：`{"rows":[]}` 是"那一次什么都没交付"的正解（Q1 的
    // 空块率判的就是这种行），把它读成 unreadable 就等于把空注入演成数据损坏。
    let Some(list) = v["rows"].as_array() else {
        out.unreadable = true;
        return out;
    };
    out.rows = list
        .iter()
        .filter_map(|item| {
            let key = item["key"].as_str()?;
            Some(ExplainRow {
                key: key.to_string(),
                share: arr4(&item["share"]),
                total: item["total"].as_i64().unwrap_or(0),
            })
        })
        .collect();
    out
}

#[cfg(test)]
mod tests;
