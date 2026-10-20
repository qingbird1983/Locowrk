//! 提示词分层（提示词分层设计 §6）：waterfall 注册序 = system 拼装序的纪律锁——
//! 静态身份段是 system 的字节前缀。
//!
//! ⚠ **S3 刀③a 与 S4 刀③c 两次订正过这个文件的职责**。S3 刀③a 删掉了拿
//! `TreeDirectoryInject` 当道具的两条用例；S4 刀③c 又把 `MemoryInject` 整线
//! 退役，于是本文件只证它现在管得住的那半：静态身份段恒为 system 的字节
//! 前缀（后续的指令文件层 / 尾部碎片只能往后追加）。
//!
//! "动态段不改写静态前缀"这条纪律的继承者仍在各注入层自己的 tests 里（如
//! `middleware/instruction_files/tests.rs` 的"已拼接段逐字节不动"）。

use cordis_core::{WaterfallDecision, WaterfallListener};
use kernel_agent::{BaseSystemInject, base_system, points};
use serde_json::json;
use session_store::Message;

async fn chain() -> String {
    let mut payload = serde_json::to_value(kernel_agent::RequestPayload {
        messages: vec![Message::user("hi")],
        model: None,
        params: json!({}),
        tools: Vec::new(),
        compacted: None,
        system: None,
        session_id: Some("s1".into()),
    })
    .unwrap();
    // 注册序即执行序（state/waterfall.rs 同序）：Base 在最前。
    //
    // 台账 T2 之后每一环就地改 payload、收束是 `Continue`。断言 `Continue`
    // 因此同时是两件事的证据：① 改写没有漂回整包 `Replace`；② 守卫真的把
    // 载荷还回来了——漏还时下游解析 `Null` 会当场红。
    let d = BaseSystemInject.handle(points::REQUEST, &mut payload).await;
    assert!(
        matches!(d, WaterfallDecision::Continue),
        "BaseSystemInject 应就地改写后 Continue，实际 {d:?}"
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
    assert!(
        system.starts_with(base_system()),
        "静态段 = system 字节前缀（其它层只能往后追加）"
    );
    assert_eq!(i_base, 0, "静态段从第 0 字节起");
}
