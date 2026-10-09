//! 提示词分层（提示词分层设计 §6）：waterfall 注册序 = system 拼装序的
//! 纪律锁——静态段在前缀、记忆/指引块居中、树目录殿后（append-only 段
//! 之后不再有静态段）；目录增长不打断任何静态段的前缀（M1 关键断言：
//! 整理一次断一次缓存的 v2.137 前排序缺陷在此杜绝）。

use std::sync::Arc;

use cordis_core::{WaterfallDecision, WaterfallListener};
use kernel_agent::{
    BaseSystemInject, MemoryEntry, MemoryInject, MemoryInjectConfig, MemorySource,
    TreeDirectoryInject, base_system, points,
};
use serde_json::json;
use session_store::Message;

#[derive(Default)]
struct EmptySource;

impl MemorySource for EmptySource {
    fn recall(&self, _s: Option<&str>, _l: usize) -> Vec<MemoryEntry> {
        Vec::new()
    }
}

async fn chain(dir: &str) -> String {
    let payload = serde_json::to_value(kernel_agent::RequestPayload {
        messages: vec![Message::user("hi")],
        model: None,
        params: json!({}),
        tools: Vec::new(),
        compacted: None,
        system: None,
        session_id: Some("s1".into()),
        tree_directory: Some(dir.into()),
    })
    .unwrap();
    // 注册序即执行序（state/mod.rs 同序）：Base → Memory → 目录殿后。
    let base = BaseSystemInject.handle(points::REQUEST, &payload).await;
    let WaterfallDecision::Replace(v) = base else {
        panic!("base replaces");
    };
    let memory = MemoryInject::new(
        Arc::new(EmptySource),
        Arc::new(std::sync::RwLock::new(MemoryInjectConfig::default())),
    )
    .handle(points::REQUEST, &v)
    .await;
    let WaterfallDecision::Replace(v) = memory else {
        panic!("memory replaces");
    };
    let dir_out = TreeDirectoryInject.handle(points::REQUEST, &v).await;
    let WaterfallDecision::Replace(v) = dir_out else {
        panic!("directory replaces");
    };
    serde_json::from_value::<kernel_agent::RequestPayload>(v)
        .unwrap()
        .system
        .unwrap()
}

#[tokio::test]
async fn static_prefix_memory_middle_directory_last() {
    let system = chain("[memory tree]\n- b1: title").await;
    let (i_base, i_guide, i_dir) = (
        system.find("You are Locowrk").expect("静态段在最前"),
        system.find("<memory_guidance>").expect("指引块居中"),
        system.find("[memory tree]").expect("目录殿后"),
    );
    assert!(
        system.starts_with(base_system()),
        "静态段 = system 字节前缀"
    );
    assert!(i_base < i_guide && i_guide < i_dir, "静态 → 记忆 → 目录");
    assert!(system.ends_with("- b1: title"), "目录之后无任何段");
}

/// 目录增长（整理追加一捆）只动尾部：前一个标记起的前缀逐字节不变——
/// 思考语言/记忆块不再随整理断缓存。
#[tokio::test]
async fn directory_growth_leaves_prefix_intact() {
    let before = chain("[memory tree]\n- b1: title").await;
    let after = chain("[memory tree]\n- b1: title\n- b2: more").await;
    let cut = after.find("[memory tree]").expect("目录标记在");
    assert_eq!(
        &before[..before.find("[memory tree]").expect("目录标记在")],
        &after[..cut],
        "目录之前的全部段逐字节一致"
    );
}
