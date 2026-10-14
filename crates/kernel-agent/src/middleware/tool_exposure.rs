//! ToolExposure（exposure 文档 §5/§6/§7）：`agent/request` waterfall 监听器
//! ——按打分截断每请求的工具目录，控制上下文开销与选择信噪比。
//!
//! 链位（exposure 文档 §7）：**ToolExposure 最先**，其后 BaseSystemInject /
//! Memory-Inject / 思考语言 / 树目录（v2.139 重排；旧文本此处并列写的
//! ctx-compact 已随审计修复批 C 删除本体）——先决定暴露多少工具，后续件
//! 才知道还有多少空间（注册次序即执行次序，组装点负责本件最先注册）。
//!
//! 打分（§5.3）：`1.0 × 会话内用过 + 0.7 × 全局 EWMA + 0.5 × 语义相似`
//! （always 满分直通不占额度）。会话热度直接从 `payload.messages`
//! 实时数 `ToolResult`（§3.5"会话内统计不落盘"——投影日志就是会话事实，
//! 无需会话句柄、无跨会话竞态）；全局热度经 [`GlobalEwma`] trait 由组装点
//! 提供（desktop 读 usage.sqlite3，同 ApprovalResolver 的"内核定契约、
//! 组装点实现"模式）。检索信号（⑬，§4.2 层次 2）经 [`Retriever`]——
//! embedding 未配置 = None，退回两层打分。打分失败按 fail-open 放行
//! 全目录，不拦聊天。
//!
//! hidden 不在本监听器的考虑范围：⑩ 的 `list_tools` 已把它们剔出目录，
//! 显式检索通路（search_tools 元工具）读全目录索引。
//!
//! 会话钉死（KV v2 §4.2，M1）：Top-N 逐轮重算会让 tools 块逐轮变化——
//! 前缀缓存从 tools 位置起全灭（头号杀手）。`pin_session` 开（缺省）时
//! 暴露集在会话首次请求确定并缓存，后续轮复用；`search_tools` 元工具
//! 按需拉取不改变已暴露集；权限模式切换改变目录 = 显式变更，接受失效。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use cordis_core::{WaterfallDecision, WaterfallListener};
use plugin_api::Exposure;
use serde::Serialize;
use serde_json::Value;

use super::tool_retrieval::Retriever;
use crate::timeutil::now_secs;
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
/// 与 ⑩ 直通态一致）；max_tools = 每请求暴露上限 N（§5.3，缺省 20）；
/// pin_session（KV v2 §4.2，缺省开）= 暴露集会话钉死——首次请求确定并
/// 缓存，后续轮复用不再逐轮重算（tools 块逐字节稳定，前缀缓存不灭）；
/// 关闭 = 恢复每轮 Top-N 旧行为（实验对比用）。钉死只作用于带会话身份
/// 的请求（桌面端全程带；无身份 = 每轮重算，一次性进程等价会话）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ToolExposureConfig {
    pub enabled: bool,
    pub max_tools: usize,
    pub pin_session: bool,
}

impl Default for ToolExposureConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_tools: 20,
            pin_session: true,
        }
    }
}

/// 钉死缓存上限（FIFO 驱逐最旧会话）：桌面长开进程的会话数量级有限，
/// 128 只是防无界增长的保险丝。
const PIN_CAP: usize = 128;

/// 一次钉死的选择：名字序（重建暴露集的顺序）+ 归因组（后续轮快照照抄
/// 建立时的归因——钉死后归因不再变化）。
#[derive(Debug, Clone)]
struct PinnedSelection {
    names: Vec<String>,
    always: Vec<String>,
    session_hot: Vec<String>,
    global_hot: Vec<String>,
    retrieved: Vec<String>,
}

/// 会话 → 钉死选择表（FIFO 驱逐）。
#[derive(Default)]
struct SessionPins {
    map: HashMap<String, PinnedSelection>,
    order: std::collections::VecDeque<String>,
}

impl SessionPins {
    fn get(&self, id: &str) -> Option<&PinnedSelection> {
        self.map.get(id)
    }

    fn insert(&mut self, id: String, sel: PinnedSelection) {
        if !self.map.contains_key(&id) {
            self.order.push_back(id.clone());
            if self.order.len() > PIN_CAP {
                if let Some(old) = self.order.pop_front() {
                    self.map.remove(&old);
                }
            }
        }
        self.map.insert(id, sel);
    }
}

/// 最近一次截断的选择快照（§6.3 调试面板数据源）：按选中原因分组，
/// token 节省为 chars/4 估算（与 `estimate_tokens` 同一把尺，§9.6 的"约"）。
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExposureSnapshot {
    pub enabled: bool,
    pub total: usize,
    pub exposed: usize,
    pub always: Vec<String>,
    pub session_hot: Vec<String>,
    pub global_hot: Vec<String>,
    /// 靠检索信号（0.5 × 相似度）入选的工具（⑬ 语义层；embedding 未
    /// 配置时恒空）。相似度是连续补充信号，这里只列贡献 > 0 的入选者。
    pub retrieved: Vec<String>,
    pub saved_tokens_estimate: u64,
    pub ts: i64,
}

// 本文件只留**监听器本体**与配置/钉死缓存/快照类型。两簇实现搬到 sibling 子模块
// （批 F-23：M-24c 给本文件加了约 60 行计量码，把 465 行推过 400 线；三问自检
// 「只做一件事 / 名字准确 / 改动不牵连」有两条为否 ⇒ 拆，而不是抬 check_length
// 基线。形状照批 F-17 的 `session.rs` + `session/project.rs` 先例）。
mod measure;
mod selection;

// 私有 use：`tests/` 子簇经 `use super::*` 的 glob 拿到这些名字（与 `persist.rs`
// 底部 `use meta::{invalid_id, meta_file}` 同族，第五道闸 S6 认这一档出处）。
// `Selection` 不在其中：字段访问不要求类型名在作用域，父与测试都只写 `sel.kept_idx`
// （clippy `--all-targets` 实测把显式引入判成 unused_imports）。
use measure::{array_json_len, spec_json_len, tokens_of};
use selection::{reorder, retrieval_query, select, session_counts};

/// 挂在 `agent/request` 上的工具暴露截断监听器。
pub struct ToolExposure {
    global: Arc<dyn GlobalEwma>,
    cfg: Arc<RwLock<ToolExposureConfig>>,
    retrieval: Option<Arc<Retriever>>,
    last: Mutex<Option<ExposureSnapshot>>,
    pins: Mutex<SessionPins>,
}

impl ToolExposure {
    /// `retrieval` = ⑬ 语义检索器（embedding 未配置传 None，退回两层打分）。
    pub fn new(
        global: Arc<dyn GlobalEwma>,
        cfg: Arc<RwLock<ToolExposureConfig>>,
        retrieval: Option<Arc<Retriever>>,
    ) -> Self {
        Self {
            global,
            cfg,
            retrieval,
            last: Mutex::new(None),
            pins: Mutex::new(SessionPins::default()),
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
        // 会话钉死命中（KV v2 §4.2）：按建立时的名字序从当前目录回映射——
        // 目录未变 = 逐字节同序同 spec（前缀稳定）；目录被围栏收紧（计划
        // 模式）或插件中途移除 = 交集，显式变更接受失效。归因组照抄建立
        // 时（钉死后归因不再变化）。
        // M4 补口（走查实抓）：目录**放宽**时新增的 always 工具（如会话
        // 中途切记忆树模式，memory_expand 进目录）无条件保留、追加在尾部
        // ——钉死钉的是 on_demand 截断决策，always 直通是 ⑩ 语义；尾部
        // 追加不破旧前缀（一次性显式失效，M4 §3.38①「切换即时生效」）。
        if cfg.pin_session {
            if let Some(id) = req.session_id.as_deref() {
                let pinned = self
                    .pins
                    .lock()
                    .expect("exposure pins poisoned")
                    .get(id)
                    .cloned();
                if let Some(pinned) = pinned {
                    // 逐项 JSON 长度只算**一遍**，保留集长度由下标子集求和得出
                    // （旧写法把全目录与保留集各整体序列化一次；M-24c）。
                    let lens: Vec<u64> = req.tools.iter().map(spec_json_len).collect();
                    let mut kept_idx: Vec<usize> = pinned
                        .names
                        .iter()
                        .filter_map(|n| req.tools.iter().position(|s| &s.name == n))
                        .collect();
                    for (i, s) in req.tools.iter().enumerate() {
                        if matches!(s.exposure, Exposure::Always) && !pinned.names.contains(&s.name)
                        {
                            kept_idx.push(i);
                        }
                    }
                    let kept_lens: Vec<u64> = kept_idx.iter().map(|&i| lens[i]).collect();
                    let saved = tokens_of(array_json_len(&lens))
                        .saturating_sub(tokens_of(array_json_len(&kept_lens)));
                    self.record(ExposureSnapshot {
                        enabled: true,
                        total: req.tools.len(),
                        exposed: kept_idx.len(),
                        always: pinned.always,
                        session_hot: pinned.session_hot,
                        global_hot: pinned.global_hot,
                        retrieved: pinned.retrieved,
                        saved_tokens_estimate: saved,
                        ts: now_secs(),
                    });
                    req.tools = reorder(&mut req.tools, &kept_idx);
                    return super::replace_decision(req);
                }
            }
        }
        let session = session_counts(&req.messages);
        let ewma = self.global.ewma_scores();
        let total = req.tools.len();
        // 检索信号只在确实要裁剪时才算（目录 ≤ 上限 = 全保留，无需为一
        // 个不影响结果的分数多打一次 embedding）；embedding 失败 → 空表
        // = 退回两层打分（§1 可降级，fail-open）
        let retrieval = match (&self.retrieval, total > cfg.max_tools) {
            (Some(r), true) => r.scores(&retrieval_query(&req.messages), &req.tools).await,
            _ => HashMap::new(),
        };
        // 逐项 JSON 长度在裁剪**之前**算一遍：全目录与保留集的长度都由它得出，
        // 不再把含 schema 的目录整体序列化两次（M-24c）。
        let lens: Vec<u64> = req.tools.iter().map(spec_json_len).collect();
        let sel = select(&req.tools, &session, &ewma, &retrieval, cfg.max_tools);
        let kept_lens: Vec<u64> = sel.kept_idx.iter().map(|&i| lens[i]).collect();
        let saved =
            tokens_of(array_json_len(&lens)).saturating_sub(tokens_of(array_json_len(&kept_lens)));
        self.record(ExposureSnapshot {
            enabled: true,
            total,
            exposed: sel.kept_idx.len(),
            always: sel.always.clone(),
            session_hot: sel.session_hot.clone(),
            global_hot: sel.global_hot.clone(),
            retrieved: sel.retrieved.clone(),
            saved_tokens_estimate: saved,
            ts: now_secs(),
        });
        // 钉死开且带会话身份：首次选择入缓存，后续轮直接复用（§4.2）
        if cfg.pin_session {
            if let Some(id) = req.session_id.clone() {
                self.pins.lock().expect("exposure pins poisoned").insert(
                    id,
                    PinnedSelection {
                        names: sel
                            .kept_idx
                            .iter()
                            .map(|&i| req.tools[i].name.clone())
                            .collect(),
                        always: sel.always,
                        session_hot: sel.session_hot,
                        global_hot: sel.global_hot,
                        retrieved: sel.retrieved,
                    },
                );
            }
        }
        req.tools = reorder(&mut req.tools, &sel.kept_idx);
        super::replace_decision(req)
    }
}

#[cfg(test)]
mod tests;
