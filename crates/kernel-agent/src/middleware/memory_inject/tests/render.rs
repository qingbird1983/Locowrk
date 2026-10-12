//! 注入块渲染（自 `tests.rs` 批 F-16 拆出，用例本体逐字未改）：事实块与指引块进
//! system / top_k 截条目 / 停用与零额度 noop / 空记忆仍有指引（F3）/ 超长值截断标记 /
//! tags 渲染 / 追加到已有 system / 非 REQUEST 点 noop / 双层条目层标 / 指引落块尾。

use super::*;

use crate::waterfall::{RequestPayload, points};
use cordis_core::{WaterfallDecision, WaterfallListener};
use serde_json::json;

#[tokio::test]
async fn injects_block_into_system() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("user.lang", "zh-CN")])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    let system = out.system.unwrap();
    assert!(system.contains("<persistent_memory>"));
    assert!(system.contains("- user.lang = zh-CN"));
    assert!(!system.contains("truncated"));
}

#[tokio::test]
async fn top_k_limits_entries() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![
            entry("a", "1"),
            entry("b", "2"),
            entry("c", "3"),
        ])),
        cfg_lock(MemoryInjectConfig {
            top_k: 2,
            ..MemoryInjectConfig::default()
        }),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    let system = out.system.unwrap();
    assert!(system.contains("- a = 1"));
    assert!(system.contains("- b = 2"));
    assert!(!system.contains("- c = 3"));
}

#[tokio::test]
async fn disabled_or_zero_top_k_is_noop() {
    for cfg in [
        MemoryInjectConfig {
            enabled: false,
            ..MemoryInjectConfig::default()
        },
        MemoryInjectConfig {
            enabled: true,
            top_k: 0,
        },
    ] {
        let inject = inject(
            std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
            cfg_lock(cfg),
        );
        assert!(run(&inject, points::REQUEST).await.is_none());
    }
}

/// 空记忆会话（F3 起）：指引块单独注入——主动记忆的本意就是零记忆的
/// 新会话也要知道"该自己记"，不该等攒出第一条记忆才被告知。
#[tokio::test]
async fn empty_recall_still_gets_guidance() {
    let inject = inject(
        std::sync::Arc::new(Fixed(Vec::new())),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.expect("指引块单独成块");
    let system = out.system.unwrap();
    assert!(system.contains("<memory_guidance>"));
    assert!(system.contains("scope=\"global\""));
    assert!(!system.contains("<persistent_memory>"), "零记忆无事实块");
}

#[tokio::test]
async fn long_values_truncated_with_marker() {
    let long = "z".repeat(600); // 指引块文本含 x（"expected"），计数填充换 z
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("big", &long)])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    let system = out.system.unwrap();
    assert!(system.contains("…(truncated)"));
    // 恰好 500 个 z + 标记，而不是 600 个
    assert_eq!(system.matches('z').count(), 500);
}

#[tokio::test]
async fn tags_rendered_when_present() {
    let mut e = entry("k", "v");
    e.tags = "pref".into();
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![e])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    assert!(out.system.unwrap().contains(" [tags: pref]"));
}

#[tokio::test]
async fn appends_to_existing_system() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let mut payload = payload_value();
    payload["system"] = json!("base instructions");
    match inject.handle(points::REQUEST, &payload).await {
        WaterfallDecision::Replace(v) => {
            let req: RequestPayload = serde_json::from_value(v).unwrap();
            let system = req.system.unwrap();
            assert!(system.starts_with("base instructions"));
            assert!(system.contains("<persistent_memory>"));
        }
        _ => panic!("expected replace"),
    }
}

#[tokio::test]
async fn non_request_key_is_noop() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    assert!(run(&inject, points::PRE_STEP).await.is_none());
}

/// 归属会话的记忆块带层标（双层记忆：全局 = 用户偏好 / 项目 = 本项目）。
#[tokio::test]
async fn scoped_entries_render_tier_prefix() {
    let mut g = entry("user.lang", "zh-CN");
    g.scope = "global".into();
    let mut p = entry("arch.note", "uses pnpm");
    p.scope = "project".into();
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![g, p])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.unwrap().system.unwrap();
    assert!(out.contains("- [global] user.lang = zh-CN"));
    assert!(out.contains("- [project] arch.note = uses pnpm"));
}

/// 主动记忆指引（F3 起，v2.139 §5.2 改写；**Q2 · MR §4.4 增两句**）：随注入块尾——
/// 在事实块之后；write 主动 / read 按需的禁令在场，Q2 两条实测订正也在场
/// （① 用户显式问过去时**必须查工具**而不是从注入上下文假设；② 空结果 ≠ 没有
/// 历史，项目历史在树里）。旧句「never as a way to start a conversation」必须
/// **不在场**——它被模型读成"关于过去的提问也别查"（MR 稿 §0-4 的 audit 动线实证）。
#[tokio::test]
async fn guidance_appended_at_block_tail() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    match inject.handle(points::REQUEST, &payload_value()).await {
        WaterfallDecision::Replace(v) => {
            let out: RequestPayload = serde_json::from_value(v).unwrap();
            let out = out.system.unwrap();
            let mem = out.find("<persistent_memory>").expect("事实块在");
            let guide = out.find("<memory_guidance>").expect("指引块在");
            assert!(mem < guide, "指引在注入块尾");
            assert!(
                out.contains("don't probe memory unprompted"),
                "read 侧按需禁令（Q2 换了措辞）: {out}"
            );
            assert!(
                !out.contains("never as a way to start"),
                "旧句必须已退役，否则改写没落: {out}"
            );
            assert!(
                out.contains("query these tools instead of assuming from injected context"),
                "Q2 第①句在场: {out}"
            );
            assert!(
                out.contains("do NOT mean there is no history")
                    && out.contains("memory_project_history first"),
                "Q2 第②句与它的指路工具点名: {out}"
            );
            assert!(
                out.contains(".Locowrk/"),
                "项目文件自维护的指引语义保留: {out}"
            );
        }
        _ => panic!("expected replace"),
    }
}
