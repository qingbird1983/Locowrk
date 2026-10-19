//! memory_inject 单测：索引段装配 / 配置开关 / 缺席形状 / 追加语义 / 非 REQUEST 点不碰。
//! 批 F-16 目录化：共享假件与跑批入口留本文件，用例按面分簇见 `render` /
//! `prefetch` / `scope_context`；Q2（MR §4.1）加 `project_index`
//! （项目记忆索引段的落点、缺席条件与随预取钉死）。
//!
//! ⚠ **S3 刀① 删掉的两个文件簇**（不是"顺手清"，是被测对象在本刀消失）：
//! `learn.rs` 整簇（它测的是"内核把相关性词表递下去 + 学习档字节回退"，那条通路随
//! `<persistent_memory>` 正文一起下线；`memory_learn/` 一族按裁定到刀③ 同刀退役）与
//! `render.rs` 里那四条**正文渲染**用例（top_k 截条目 / 超长截断标记 / tags 渲染 /
//! 层标前缀——注入面不再有正文，这四件事没有对象了）。
//! 「同会话只取数一次」这条 M1 承重从 learn.rs 移到 `prefetch.rs`（道具换成索引内文）。

mod prefetch;
mod project_index;
mod render;
mod scope_context;

use super::{MemoryEntry, MemoryInject, MemoryInjectConfig, MemorySource};
use crate::waterfall::{RequestPayload, points};
use cordis_core::{WaterfallDecision, WaterfallListener};

/// 什么都不提供的源：`recall` 交空表，其余方法吃 trait 缺省（全 `None`）。
/// 它是"只有指引块"那一格的道具——S3 刀① 之后注入环不再调 `recall`，
/// 但 trait 把它列为必需方法，所以这里仍要给一个实现。
struct Empty;

impl MemorySource for Empty {
    fn recall(&self, _session_id: Option<&str>, _limit: usize) -> Vec<MemoryEntry> {
        Vec::new()
    }
}

/// 给得出两层索引内文的源（S3 刀① 的主道具）：`calls` 记**两段一共被问了几次**，
/// 是"每会话至多取数一次"这条 M1 承重的读数；两个 `Mutex` 让用例能在装进监听器
/// **之后**再改内文——"中途真变了也不许改写已钉死的会话"那一格要靠它。
pub struct Sourced {
    global: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    project: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
}

/// 一段索引内文的句柄（用例要能在源被搬进监听器**之后**再改它）。
type Section = std::sync::Arc<std::sync::Mutex<Option<String>>>;
/// 取数计数句柄（"每会话至多问宿主一次"的读数）。
type Calls = std::sync::Arc<std::sync::atomic::AtomicUsize>;

impl Sourced {
    pub fn new(global: Option<&str>, project: Option<&str>) -> Self {
        Self {
            global: std::sync::Arc::new(std::sync::Mutex::new(global.map(str::to_string))),
            project: std::sync::Arc::new(std::sync::Mutex::new(project.map(str::to_string))),
            calls: std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// 三枚句柄（内文两枚 + 取数计数一枚），用例改内文与读计数都靠它。
    ///
    /// 返回类型抽了 `Section` / `Calls` 两个别名：不是怕 clippy 的 `type_complexity`，
    /// 是三枚 `Arc<Mutex<Option<String>>>` 摊在签名里，读的人得分辨哪一枚是哪一段。
    pub fn handles(&self) -> (Section, Section, Calls) {
        (
            std::sync::Arc::clone(&self.global),
            std::sync::Arc::clone(&self.project),
            std::sync::Arc::clone(&self.calls),
        )
    }
}

impl MemorySource for Sourced {
    fn recall(&self, _session_id: Option<&str>, _limit: usize) -> Vec<MemoryEntry> {
        Vec::new()
    }

    fn global_index(&self, _session_id: Option<&str>) -> Option<String> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.global.lock().unwrap().clone()
    }

    fn project_index(&self, _session_id: Option<&str>) -> Option<String> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        self.project.lock().unwrap().clone()
    }
}

fn cfg_lock(c: MemoryInjectConfig) -> std::sync::Arc<std::sync::RwLock<MemoryInjectConfig>> {
    std::sync::Arc::new(std::sync::RwLock::new(c))
}

fn payload_value() -> serde_json::Value {
    serde_json::to_value(RequestPayload {
        messages: Vec::new(),
        model: None,
        params: serde_json::Value::Object(serde_json::Map::new()),
        tools: Vec::new(),
        compacted: None,
        system: None,
        session_id: None,
        tree_directory: None,
    })
    .unwrap()
}

fn inject(
    source: std::sync::Arc<dyn MemorySource>,
    cfg: std::sync::Arc<std::sync::RwLock<MemoryInjectConfig>>,
) -> MemoryInject {
    MemoryInject::new(source, cfg)
}

/// 跑一环并回传载荷。`None` = 本环没改过一个字节。
///
/// 判据原来取自决策（`Replace` = 改过），台账 T2 之后注入环就地写、收束恒为
/// `Continue`，只能比进出的载荷。输入由 `to_value(RequestPayload)` 造 ⇒ round-trip
/// 幂等 ⇒ 字节差只可能是业务改写。
async fn pass(
    inject: &MemoryInject,
    key: &str,
    input: serde_json::Value,
) -> Option<RequestPayload> {
    let mut v = input.clone();
    match inject.handle(key, &mut v).await {
        WaterfallDecision::Continue => {}
        other => panic!("unexpected decision {other:?}"),
    }
    (v != input).then(|| serde_json::from_value(v).unwrap())
}

async fn run(inject: &MemoryInject, key: &str) -> Option<RequestPayload> {
    pass(inject, key, payload_value()).await
}

/// 带会话身份的请求。
async fn run_session(inject: &MemoryInject, session_id: &str) -> Option<RequestPayload> {
    let mut v = payload_value();
    v["session_id"] = serde_json::json!(session_id);
    pass(inject, points::REQUEST, v).await
}
