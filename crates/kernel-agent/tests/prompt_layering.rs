//! 提示词分层（提示词分层设计 §6）：waterfall 注册序 = system 拼装序的纪律锁——
//! 静态身份段是 system 的字节前缀，记忆块随后。
//!
//! ⚠ **S3 刀③a 订正过这个文件的职责**。原先三条用例全拿 `TreeDirectoryInject` 当道具，
//! 证的是"append-only 目录殿后 ⇒ 目录增长不打断任何静态段前缀"（v2.137 那次
//! "整理一次断一次缓存"的排序缺陷就靠它杜绝）。树目录注入随方案稿 §C 退役后，
//! "本环之后不再有静态段"这个前提已经不成立（指令文件层还在后面），所以：
//! - `directory_growth_leaves_prefix_intact` 与 `empty_directory_appends_nothing`
//!   两条随道具一起删除——它们断言的那个东西不在生产面上了，不是为过闸删测试；
//! - "动态段不改写静态前缀"这条纪律的**继承者**写在
//!   `memory_inject/tests/prefetch.rs` 与 `project_index.rs`（同会话逐字节复用 +
//!   每会话至多取数一次 + 新鲜度档变了才重算）。留在这里的这条只证它还管得住的那半：
//!   静态身份段恒为 system 的字节前缀。

use std::sync::Arc;

use cordis_core::{WaterfallDecision, WaterfallListener};
use kernel_agent::{
    BaseSystemInject, MemoryEntry, MemoryInject, MemoryInjectConfig, MemorySource, base_system,
    points,
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

async fn chain() -> String {
    let mut payload = serde_json::to_value(kernel_agent::RequestPayload {
        messages: vec![Message::user("hi")],
        model: None,
        params: json!({}),
        tools: Vec::new(),
        compacted: None,
        system: None,
        session_id: Some("s1".into()),
        tree_directory: None,
    })
    .unwrap();
    // 注册序即执行序（state/waterfall.rs 同序）：Base → Memory。
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
    serde_json::from_value::<kernel_agent::RequestPayload>(payload)
        .unwrap()
        .system
        .unwrap()
}

#[tokio::test]
async fn static_identity_stays_the_byte_prefix() {
    let system = chain().await;
    let i_base = system.find("You are Locowrk").expect("静态段在最前");
    let i_guide = system.find("<memory_guidance>").expect("指引块在场");
    assert!(
        system.starts_with(base_system()),
        "静态段 = system 字节前缀（记忆块只能往后追加）"
    );
    assert_eq!(i_base, 0, "静态段从第 0 字节起");
    assert!(i_base < i_guide, "静态 → 记忆");
    assert!(
        !system.contains("[memory tree]"),
        "树目录注入已随刀③a 退役: {system}"
    );
}
