//! Q2（MR 稿 §4.1）`<project_memory_index>` 在**内核这一侧**的判据：
//! 块内位置、缺席条件、与会话预取同吃钉死。
//!
//! 两档数据源（md 优先 / sqlite 兜底）的取舍归宿主（`memory_index` 与
//! `memory/tests.rs`），内核只管"宿主给文本我就包标签摆位置，不给就整节缺席"。
//! 所以这里测**装配与钉死**，不重复宿主的合成逻辑。

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use cordis_core::{WaterfallDecision, WaterfallListener};

use super::{
    MemoryEntry, MemoryInject, MemoryInjectConfig, MemorySource, RequestPayload, cfg_lock, entry,
    inject, payload_value,
};
use crate::waterfall::points;

/// 带索引内文的假件源；`calls` 记 `project_index` 被问了几次（钉死判据的读数）。
struct Sourced {
    index: Arc<Mutex<Option<String>>>,
    calls: Arc<AtomicUsize>,
}

impl Sourced {
    fn new(text: Option<&str>) -> Self {
        Self {
            index: Arc::new(Mutex::new(text.map(str::to_string))),
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
    /// 包着同一份 `index`/`calls` 的克隆句柄（测试要在装进监听器之后仍能改与读）。
    fn handle(&self) -> (Arc<Mutex<Option<String>>>, Arc<AtomicUsize>) {
        (Arc::clone(&self.index), Arc::clone(&self.calls))
    }
}

impl MemorySource for Sourced {
    fn recall(&self, _session_id: Option<&str>, limit: usize) -> Vec<MemoryEntry> {
        (0..limit.min(1))
            .map(|_| entry("user.lang", "中文"))
            .collect()
    }

    fn project_index(&self, _session_id: Option<&str>) -> Option<String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.index.lock().unwrap().clone()
    }
}

fn listener(src: Sourced) -> (MemoryInject, Arc<Mutex<Option<String>>>, Arc<AtomicUsize>) {
    let (index, calls) = src.handle();
    let inject = inject(
        Arc::new(src),
        cfg_lock(MemoryInjectConfig {
            enabled: true,
            top_k: 5,
        }),
    );
    (inject, index, calls)
}

/// 带会话身份地跑一次，取回最终 system 文本。
async fn system_of(inject: &MemoryInject, session_id: &str) -> String {
    let mut v = payload_value();
    v["session_id"] = serde_json::json!(session_id);
    match inject.handle(points::REQUEST, &v).await {
        WaterfallDecision::Replace(v) => serde_json::from_value::<RequestPayload>(v)
            .expect("payload 可解")
            .system
            .unwrap_or_default(),
        WaterfallDecision::Continue => String::new(),
        _ => panic!("记忆件只会 Replace 或 Continue"),
    }
}

#[tokio::test]
async fn index_section_sits_after_the_facts_and_before_the_guidance() {
    let (inject, _, _) = listener(Sourced::new(Some(
        "- [build.cmd](build.cmd.md) — pnpm tauri dev",
    )));
    let system = system_of(&inject, "s1").await;
    let facts = system.find("<persistent_memory>").expect("事实块在");
    let index = system
        .find("<project_memory_index>")
        .expect("索引块在（稿 §4.1 的落点）");
    let guide = system.find("<memory_guidance>").expect("指引块在");
    assert!(facts < index, "索引必须在事实块之后: {system}");
    assert!(index < guide, "索引必须在指引之前: {system}");
    assert!(
        system.contains("- [build.cmd](build.cmd.md)"),
        "宿主给的内文原样进块: {system}"
    );
    // 标签由内核包（宿主只交内文）：开闭必须成对，命名权在装配点
    assert!(
        system.contains("</project_memory_index>"),
        "闭合标签由内核补: {system}"
    );
}

#[tokio::test]
async fn no_index_from_the_host_means_no_section_at_all() {
    // 未归属 / 项目层空 / headless = 宿主给 `None` ⇒ **整节缺席**。不许留空标签：
    // `<project_memory_index></project_memory_index>` 会被读成"有索引但没条目"，
    // 与"这个项目没有索引可言"是两个不同结论（同 Q1a 的 None≠0.0 纪律）。
    let (inject, _, _) = listener(Sourced::new(None));
    let system = system_of(&inject, "s1").await;
    assert!(!system.contains("project_memory_index"), "{system}");
    assert!(
        system.contains("<persistent_memory>"),
        "其余块照旧: {system}"
    );
    assert!(system.contains("<memory_guidance>"), "指引照旧: {system}");
}

#[tokio::test]
async fn index_is_frozen_with_the_prefetched_block() {
    // **字节钉死**（M1 纪律 / KV v2 纪律 2）：索引段与记忆块同吃会话预取缓存，
    // 二次请求既不该换字节、也不该再问宿主一次。
    let (inject, index, calls) = listener(Sourced::new(Some("- a — 甲")));
    let first = system_of(&inject, "s1").await;
    let second = system_of(&inject, "s1").await;
    assert_eq!(first, second, "会话内逐字节复用");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "每会话至多问宿主一次");

    // 中途宿主侧真变了（M2 落库 / 管理页编辑触发重算）：已钉死的会话**不跟着变**，
    // 这正是设计目的——中途写库不许打断前缀缓存。
    *index.lock().unwrap() = Some("- c — 丙".to_string());
    let third = system_of(&inject, "s1").await;
    assert_eq!(third, first, "会话内不许被中途变化改写");
    assert_eq!(calls.load(Ordering::SeqCst), 1, "命中缓存后不再取数");

    // 另一个会话是另一个现场：它自己取一次，拿的是**当时**的内文
    let (inject2, _, calls2) = listener(Sourced::new(Some("- b — 乙")));
    let other = system_of(&inject2, "s2").await;
    assert!(other.contains("- b — 乙"), "{other}");
    assert_eq!(calls2.load(Ordering::SeqCst), 1, "缓存按会话分条目");
}

#[tokio::test]
async fn disabled_injection_never_asks_for_the_index() {
    // `memory.enabled=false` = 整块（含索引与指引）一起缺席，也**一次都不取数**——
    // 关到到底，别留一条只能由索引独占的读库口子。
    let src = Sourced::new(Some("- a — 甲"));
    let calls = Arc::clone(&src.calls);
    let inject = inject(
        Arc::new(src),
        cfg_lock(MemoryInjectConfig {
            enabled: false,
            top_k: 5,
        }),
    );
    match inject.handle(points::REQUEST, &payload_value()).await {
        WaterfallDecision::Continue => {}
        _ => panic!("关闭后不该改写 payload"),
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0, "关了就一个字节都不取");
}

#[tokio::test]
async fn other_waterfall_points_never_take_the_index() {
    // 索引只属于 `agent/request`（system 装配点）。其它点零参与，也不该顺手取数。
    let src = Sourced::new(Some("- a — 甲"));
    let calls = Arc::clone(&src.calls);
    let inject = inject(Arc::new(src), cfg_lock(MemoryInjectConfig::default()));
    for key in [points::PRE_STEP, points::REQUEST_ERROR, "agent/respond"] {
        match inject.handle(key, &payload_value()).await {
            WaterfallDecision::Continue => {}
            _ => panic!("非 REQUEST 点不该参与: {key}"),
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0, "零次取数");
}
