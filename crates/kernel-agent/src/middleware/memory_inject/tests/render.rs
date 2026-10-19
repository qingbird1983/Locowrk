//! 注入块装配（S3 刀① 重写）：索引块两段进 system / 单段在场仍成块 / 两段都缺席时
//! **不留空标签** / 停用 noop / 零记忆仍有指引（F3）/ 追加到已有 system / 非 REQUEST 点 noop。
//!
//! 旧形状里这一簇还测正文渲染的四件事（top_k 截条目 / 超长值截断标记 / tags 渲染 /
//! 层标前缀）。它们随 `<persistent_memory>` 一起消失——注入面不再有正文，这四件事
//! **没有对象**，不是"改成测索引"能替代的（删掉的理由记在 `tests/mod.rs` 头注）。

use super::*;

use crate::waterfall::points;
use serde_json::json;

const GLOBAL_LINE: &str = "- [user.lang](user.lang.md) — 中文沟通";
const PROJECT_LINE: &str = "- [build.cmd](build.cmd.md) — pnpm tauri dev";

#[tokio::test]
async fn index_block_carries_both_sections_in_layer_order() {
    let inject = inject(
        std::sync::Arc::new(Sourced::new(Some(GLOBAL_LINE), Some(PROJECT_LINE))),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST)
        .await
        .expect("有索引就该改写 payload");
    let system = out.system.unwrap();
    assert!(system.contains("<memory_index>"), "{system}");
    assert!(
        system.contains(&format!("[global]\n{GLOBAL_LINE}")),
        "全局段：段头 + 宿主给的内文原样进块: {system}"
    );
    assert!(
        system.contains(&format!("[project]\n{PROJECT_LINE}")),
        "项目段: {system}"
    );
    assert!(
        system.contains("</memory_index>"),
        "闭合标签由内核补（命名权在装配点）: {system}"
    );
    assert!(
        !system.contains("<persistent_memory>") && !system.contains("<project_memory_index>"),
        "旧形状那两块必须彻底不在场: {system}"
    );
    assert!(
        system.find("[global]").unwrap() < system.find("[project]").unwrap(),
        "全局段排在项目段之前（层序 = 由宽到窄）: {system}"
    );
}

/// 只有一段在场 ⇒ 块仍然生成，但**只包那一段**。两段的缺席条件各自独立：
/// 未归属会话没有"本项目"可言，而全局层照旧有货。
#[tokio::test]
async fn a_single_section_still_wraps_the_block() {
    let inject = inject(
        std::sync::Arc::new(Sourced::new(Some(GLOBAL_LINE), None)),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let system = run(&inject, points::REQUEST).await.unwrap().system.unwrap();
    assert!(system.contains("<memory_index>"));
    assert!(system.contains(GLOBAL_LINE));
    assert!(
        !system.contains("[project]"),
        "没有的那一段不留段头: {system}"
    );
}

/// 两段都拿不到 ⇒ **整块缺席**。不许留一对空标签：`<memory_index>\n</memory_index>`
/// 会被模型读成"有索引但没条目"，与"这两层都没记过"是两个不同结论
/// （与刀① 之前那条 `None` ≠ 空块的纪律同一条）。
#[tokio::test]
async fn no_sections_means_no_index_block_at_all() {
    let inject = inject(
        std::sync::Arc::new(Sourced::new(None, None)),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let system = run(&inject, points::REQUEST)
        .await
        .expect("指引块仍然注入")
        .system
        .unwrap();
    assert!(!system.contains("memory_index"), "{system}");
    assert!(system.contains("<memory_guidance>"), "指引照旧: {system}");
}

/// 零记忆的会话（F3 起）：指引块单独注入——主动记忆的本意就是零记忆的
/// 新会话也要知道"该自己记"，不该等攒出第一条记忆才被告知。
#[tokio::test]
async fn empty_source_still_gets_guidance() {
    let inject = inject(
        std::sync::Arc::new(Empty),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.expect("指引块单独成块");
    let system = out.system.unwrap();
    assert!(system.contains("<memory_guidance>"));
    assert!(system.contains("scope=\"global\""));
    assert!(!system.contains("<memory_index>"), "零索引无索引块");
}

/// 指引落在索引块**之后**：先告诉模型"记了哪些条"，再告诉它"该怎么记、怎么读"。
#[tokio::test]
async fn guidance_appended_after_the_index_block() {
    let inject = inject(
        std::sync::Arc::new(Sourced::new(Some(GLOBAL_LINE), Some(PROJECT_LINE))),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let system = run(&inject, points::REQUEST).await.unwrap().system.unwrap();
    let index = system.find("<memory_index>").expect("索引块在");
    let guide = system.find("<memory_guidance>").expect("指引块在");
    assert!(index < guide, "指引在注入块尾: {system}");
    assert!(
        system.contains("don't probe memory unprompted"),
        "read 侧按需禁令在场: {system}"
    );
}

/// `enabled = false` = 整块（含索引与指引）一起缺席，而且**一次都不取数**——
/// 关到到底，别留一条只能由索引独占的读库口子。
#[tokio::test]
async fn disabled_injection_is_noop_and_never_takes_data() {
    let src = Sourced::new(Some(GLOBAL_LINE), Some(PROJECT_LINE));
    let (_, _, calls) = src.handles();
    let inject = inject(
        std::sync::Arc::new(src),
        cfg_lock(MemoryInjectConfig {
            enabled: false,
            ..MemoryInjectConfig::default()
        }),
    );
    assert!(
        run(&inject, points::REQUEST).await.is_none(),
        "关闭后 payload 一字未动"
    );
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        0,
        "关了就一个字节都不取"
    );
}

/// S3 刀① 的早退口径订正：旧形状里 `top_k == 0` 会把整块关掉（那时它数的是**正文
/// 条数**）。索引常驻之后没有"条数"可数，所以 `top_k = 0` **不再**拦注入——
/// 这一条钉的就是这次语义变更本身。
#[tokio::test]
async fn zero_top_k_no_longer_switches_the_block_off() {
    let inject = inject(
        std::sync::Arc::new(Sourced::new(Some(GLOBAL_LINE), Some(PROJECT_LINE))),
        cfg_lock(MemoryInjectConfig {
            enabled: true,
            top_k: 0,
            ..MemoryInjectConfig::default()
        }),
    );
    let system = run(&inject, points::REQUEST)
        .await
        .expect("top_k = 0 不再等于关掉注入")
        .system
        .unwrap();
    assert!(system.contains(GLOBAL_LINE), "{system}");
}

#[tokio::test]
async fn appends_to_existing_system() {
    let inject = inject(
        std::sync::Arc::new(Sourced::new(Some(GLOBAL_LINE), Some(PROJECT_LINE))),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let mut v = payload_value();
    v["system"] = json!("base instructions");
    let system = pass(&inject, points::REQUEST, v)
        .await
        .expect("这一环改写过 payload")
        .system
        .expect("system 在场");
    assert!(system.starts_with("base instructions"));
    assert!(system.contains("<memory_index>"));
}

#[tokio::test]
async fn non_request_key_is_noop() {
    let inject = inject(
        std::sync::Arc::new(Sourced::new(Some(GLOBAL_LINE), Some(PROJECT_LINE))),
        cfg_lock(MemoryInjectConfig::default()),
    );
    assert!(run(&inject, points::PRE_STEP).await.is_none());
}
