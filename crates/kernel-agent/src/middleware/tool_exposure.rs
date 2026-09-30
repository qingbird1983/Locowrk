//! ToolExposure（exposure 文档 §5/§6/§7）：`agent/request` waterfall 监听器
//! ——按打分截断每请求的工具目录，控制上下文开销与选择信噪比。
//!
//! 链位（exposure 文档 §7）：**ToolExposure → Memory-Inject → ctx-compact**
//! ——先决定暴露多少工具，压缩才知道要保留多少空间（注册次序即执行次序，
//! 组装点负责本件最先注册）。
//!
//! 打分（§5.3）：`1.0 × 会话内用过 + 0.7 × 全局 EWMA`（检索 0.5 归 ⑬
//! 向量层；always 满分直通不占额度）。会话热度直接从 `payload.messages`
//! 实时数 `ToolResult`（§3.5"会话内统计不落盘"——投影日志就是会话事实，
//! 无需会话句柄、无跨会话竞态）；全局热度经 [`GlobalEwma`] trait 由组装点
//! 提供（desktop 读 usage.sqlite3，同 ApprovalResolver 的"内核定契约、
//! 组装点实现"模式）。打分失败按 fail-open 放行全目录，不拦聊天。
//!
//! hidden 不在本监听器的考虑范围：⑩ 的 `list_tools` 已把它们剔出目录，
//! 显式检索通路（search_tools 元工具）读全目录索引。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use cordis_core::{WaterfallDecision, WaterfallListener};
use plugin_api::{Exposure, ToolSpec};
use serde::Serialize;
use serde_json::Value;
use session_store::Message;

use crate::waterfall::{RequestPayload, points};

/// 会话内权重（§5.3：本会话用过 → 至少 +1.0，基本稳进）。
pub const SESSION_WEIGHT: f64 = 1.0;
/// 全局热度权重（§5.3：全局热最高 +0.7，需要配合其他信号）。
pub const GLOBAL_WEIGHT: f64 = 0.7;

/// 全局热度契约：工具名 → EWMA 分（decay = exp(-days/7)，半衰期约 5 天，
/// §5.1；积累在组装点的使用统计库）。同步短查询，任何失败返回空表——
/// 没有全局热度只是退回"会话内优先"，截断本身照常工作。
pub trait GlobalEwma: Send + Sync + 'static {
    fn ewma_scores(&self) -> HashMap<String, f64>;
}

/// 截断参数（settings.json `tools` 区）：enabled 停用 = 全目录直通（行为
/// 与 ⑩ 直通态一致）；max_tools = 每请求暴露上限 N（§5.3，缺省 20）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolExposureConfig {
    pub enabled: bool,
    pub max_tools: usize,
}

impl Default for ToolExposureConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_tools: 20,
        }
    }
}

/// 最近一次截断的选择快照（§6.3 调试面板数据源）：按选中原因分组，
/// token 节省为 chars/4 估算（与 ctx-compact 同口径，§9.6 的"约"）。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExposureSnapshot {
    pub enabled: bool,
    pub total: usize,
    pub exposed: usize,
    pub always: Vec<String>,
    pub session_hot: Vec<String>,
    pub global_hot: Vec<String>,
    pub saved_tokens_estimate: u64,
    pub ts: i64,
}

/// 一次选择的结果（纯函数产出，单测友好）。
struct Selection {
    kept: Vec<ToolSpec>,
    always: Vec<String>,
    session_hot: Vec<String>,
    global_hot: Vec<String>,
}

/// 会话内各工具的调用次数：从投影消息数 `ToolResult`（§3.5——调用意图
/// 以闭合结果计，错误结果也算用过；回填丢失工具名的悬空条目不计）。
fn session_counts(messages: &[Message]) -> HashMap<String, u32> {
    let mut counts: HashMap<String, u32> = HashMap::new();
    for m in messages {
        if let Message::ToolResult { tool, .. } = m {
            if !tool.is_empty() {
                *counts.entry(tool.clone()).or_insert(0) += 1;
            }
        }
    }
    counts
}

fn spec_tokens(specs: &[ToolSpec]) -> u64 {
    serde_json::to_string(specs).map_or(0, |s| s.len() as u64) / 4
}

/// 打分选择（§6.2）：always 无条件入选；候选按 `会话 1.0 + 全局 0.7 ×
/// ewma` 排序取前 `quota = max_tools - always` 个。同分保持目录原序
/// （stable sort，目录序 = 注册表确定性序）。选中归因：会话用过记
/// session_hot，否则全局分 > 0 记 global_hot，纯补位的不归组。
fn select(
    catalog: &[ToolSpec],
    session: &HashMap<String, u32>,
    ewma: &HashMap<String, f64>,
    max_tools: usize,
) -> Selection {
    let (always_specs, candidates): (Vec<&ToolSpec>, Vec<&ToolSpec>) = catalog
        .iter()
        .partition(|s| matches!(s.exposure, Exposure::Always));
    let quota = max_tools.saturating_sub(always_specs.len());

    let mut scored: Vec<(&ToolSpec, f64)> = candidates
        .into_iter()
        .map(|s| {
            let hit = session.contains_key(&s.name);
            let score = SESSION_WEIGHT * u32::from(hit) as f64
                + GLOBAL_WEIGHT * ewma.get(&s.name).copied().unwrap_or(0.0);
            (s, score)
        })
        .collect();
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut kept: Vec<ToolSpec> = always_specs.iter().map(|s| (*s).clone()).collect();
    let always: Vec<String> = kept.iter().map(|s| s.name.clone()).collect();
    let mut session_hot = Vec::new();
    let mut global_hot = Vec::new();
    for (spec, score) in scored.into_iter().take(quota) {
        if session.contains_key(&spec.name) {
            session_hot.push(spec.name.clone());
        } else if score > 0.0 {
            global_hot.push(spec.name.clone());
        }
        kept.push(spec.clone());
    }
    Selection {
        kept,
        always,
        session_hot,
        global_hot,
    }
}

/// 挂在 `agent/request` 上的工具暴露截断监听器。
pub struct ToolExposure {
    global: Arc<dyn GlobalEwma>,
    cfg: Arc<RwLock<ToolExposureConfig>>,
    last: Mutex<Option<ExposureSnapshot>>,
}

impl ToolExposure {
    pub fn new(global: Arc<dyn GlobalEwma>, cfg: Arc<RwLock<ToolExposureConfig>>) -> Self {
        Self {
            global,
            cfg,
            last: Mutex::new(None),
        }
    }

    fn config(&self) -> ToolExposureConfig {
        self.cfg.read().map(|c| *c).unwrap_or_default()
    }

    /// 最近一次截断快照（调试命令读）；从未截断过返回全默认快照。
    pub fn snapshot(&self) -> ExposureSnapshot {
        self.last
            .lock()
            .expect("exposure snapshot poisoned")
            .clone()
            .unwrap_or_default()
    }

    fn record(&self, snap: ExposureSnapshot) {
        *self.last.lock().expect("exposure snapshot poisoned") = Some(snap);
    }
}

#[async_trait]
impl WaterfallListener for ToolExposure {
    async fn handle(&self, key: &str, payload: &Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        let cfg = self.config();
        let Ok(mut req) = serde_json::from_value::<RequestPayload>(payload.clone()) else {
            return WaterfallDecision::Continue;
        };
        // 停用 / 上限 0：直通全目录（fail-open，行为与 ⑩ 直通态一致）
        if !cfg.enabled || cfg.max_tools == 0 {
            self.record(ExposureSnapshot {
                enabled: false,
                total: req.tools.len(),
                exposed: req.tools.len(),
                ts: now_secs(),
                ..ExposureSnapshot::default()
            });
            return WaterfallDecision::Continue;
        }
        let session = session_counts(&req.messages);
        let ewma = self.global.ewma_scores();
        let total = req.tools.len();
        let sel = select(&req.tools, &session, &ewma, cfg.max_tools);
        let saved = spec_tokens(&req.tools).saturating_sub(spec_tokens(&sel.kept));
        self.record(ExposureSnapshot {
            enabled: true,
            total,
            exposed: sel.kept.len(),
            always: sel.always,
            session_hot: sel.session_hot,
            global_hot: sel.global_hot,
            saved_tokens_estimate: saved,
            ts: now_secs(),
        });
        req.tools = sel.kept;
        // to_value 失败按 fail-open 放行（与 Memory-Inject / ctx-compact 同纪律）
        match serde_json::to_value(&req) {
            Ok(v) => WaterfallDecision::Replace(v),
            Err(_) => WaterfallDecision::Continue,
        }
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
