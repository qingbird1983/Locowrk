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
    let mut payload = serde_json::to_value(kernel_agent::RequestPayload {
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
    //
    // 台账 T2 之后每一环就地改 payload、收束是 `Continue`。逐环断言 `Continue`
    // 因此同时是两件事的证据：① 改写没有漂回整包 `Replace`；② 上一环的守卫
    // 真的把载荷还回来了——任何一环漏还，下一环 `take_request` 解析 `Null`
    // 会返回 `Reject`，这里当场红。
    let ring = |name: &str, d: WaterfallDecision| {
        assert!(
            matches!(d, WaterfallDecision::Continue),
            "{name} 应就地改写后 Continue，实际 {d:?}"
        );
    };
    ring(
        "BaseSystemInject",
        BaseSystemInject.handle(points::REQUEST, &mut payload).await,
    );
    let memory = MemoryInject::new(
        Arc::new(EmptySource),
        Arc::new(std::sync::RwLock::new(MemoryInjectConfig::default())),
    );
    ring(
        "MemoryInject",
        memory.handle(points::REQUEST, &mut payload).await,
    );
    ring(
        "TreeDirectoryInject",
        TreeDirectoryInject
            .handle(points::REQUEST, &mut payload)
            .await,
    );
    serde_json::from_value::<kernel_agent::RequestPayload>(payload)
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

/// 目录 = 空串（`Some("")` = 树会话尚无捆）：marker 仍随 payload 走，但一个字
/// 都不追加（台账 T2 把 `.clone()` + `is_empty()` 收成 `as_deref().filter(...)`，
/// 这条钉住那次形状改动没漂语义——空段会在 system 尾部留孤立换行）。
#[tokio::test]
async fn empty_directory_appends_nothing() {
    let bare = chain("").await;
    assert!(
        !bare.ends_with('\n') && !bare.ends_with("  "),
        "空目录不留尾部空白: {:?}",
        bare.chars().rev().take(8).collect::<String>()
    );
    let with_dir = chain("[memory tree]\n- b1: title").await;
    assert!(
        with_dir.starts_with(bare.as_str()),
        "有目录 = 在无目录结果的**尾部**追加，前面各段逐字节不变"
    );
}
