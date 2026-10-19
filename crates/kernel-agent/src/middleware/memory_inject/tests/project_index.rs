//! 索引段与**会话预取同吃钉死**这一面（Q2 · MR §4.1 起，S3 刀① 改成两层一段一块）。
//!
//! 装配与缺席条件在 `render.rs`；这里只测"字节钉死"——它才是 M1 纪律（KV v2 纪律 2）
//! 在索引面上的落点：索引中途变化（写库后 reconcile / 管理页编辑触发重算）如果改写
//! 本会话的 system 前缀，前缀缓存命中当场归零。
//!
//! 两档数据源的取舍与钳制归宿主（`memory_index` 与 `memory/tests/*`），内核只管
//! "宿主给文本我就包标签摆位置，不给就整段缺席"。所以这里测**装配与钉死**，
//! 不重复宿主的合成逻辑。

use std::sync::atomic::Ordering;

use super::*;

/// 带会话身份跑一次，取回最终 system 文本（未注入 = 空串）。
async fn system_of(inject: &MemoryInject, session_id: &str) -> String {
    run_session(inject, session_id)
        .await
        .map(|p| p.system.unwrap_or_default())
        .unwrap_or_default()
}

fn listener(src: Sourced) -> MemoryInject {
    inject(
        std::sync::Arc::new(src),
        cfg_lock(MemoryInjectConfig::default()),
    )
}

#[tokio::test]
async fn index_is_frozen_with_the_prefetched_block() {
    let src = Sourced::new(Some("- g — 全局"), Some("- p — 项目"));
    let (g, p, calls) = src.handles();
    let inject = listener(src);
    let first = system_of(&inject, "s1").await;
    let second = system_of(&inject, "s1").await;
    assert_eq!(first, second, "会话内逐字节复用");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "每会话至多问宿主一次：两段各一次，三轮下来仍是 2"
    );

    // 中途宿主侧真变了（M2 落库 / 管理页编辑触发重算）：已钉死的会话**不跟着变**，
    // 这正是设计目的——中途变化不许打断前缀缓存。
    *g.lock().unwrap() = Some("- z — 变了".to_string());
    *p.lock().unwrap() = Some("- y — 也变了".to_string());
    let third = system_of(&inject, "s1").await;
    assert_eq!(third, first, "会话内不许被中途变化改写");
    assert_eq!(calls.load(Ordering::SeqCst), 2, "命中缓存后不再取数");

    // 另一个会话是另一个现场：它自己取一次，拿的是**当时**的内文
    let src2 = Sourced::new(None, Some("- other — 乙"));
    let calls2 = src2.handles().2;
    let other = system_of(&listener(src2), "s2").await;
    assert!(other.contains("- other — 乙"), "{other}");
    assert!(
        !other.contains("[global]"),
        "那一段缺席就不该有段头: {other}"
    );
    assert_eq!(
        calls2.load(Ordering::SeqCst),
        2,
        "缓存按会话分条目：新会话自己把两段各问一次"
    );
}

#[tokio::test]
async fn other_waterfall_points_never_take_the_index() {
    // 索引只属于 `agent/request`（system 装配点）。其它点零参与，也不该顺手取数。
    let src = Sourced::new(Some("- g — 全局"), Some("- p — 项目"));
    let calls = src.handles().2;
    let inject = listener(src);
    for key in [points::PRE_STEP, points::REQUEST_ERROR, "agent/respond"] {
        assert!(
            pass(&inject, key, payload_value()).await.is_none(),
            "非 REQUEST 点不该参与: {key}"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0, "零次取数");
}
