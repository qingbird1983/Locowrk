//! Provider 钉模与展示视图测试（自 `llm.rs` 批 F-13 拆出，用例本体逐字未改）：provider
//! 模型钉档往返与旧缺省兼容 / 合成视图钉档胜过目录 / 自定义 Header 保留字与畸形拒绝。

use super::*;

// ---- 批 A：钉死字段幻影兼容 + 目录合成投影 ----

/// 旧 settings.json 的模型条目无 maxOutput/supportsImage 字段 = 读得进且
/// 走缺省（字段级缺省零迁移，enabled 同款）；新字段保存往返不丢。
#[test]
fn provider_model_pins_roundtrip_and_legacy_default() {
    let root = temp_root("llm-pins-legacy");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join(FILE_NAME),
        r#"{"llm":{"active":"a","providers":[{"id":"a","label":"A","kind":"openai","model":"m1","models":[{"id":"m1","alias":"别名","contextWindow":131072}]}]}}"#,
    )
    .unwrap();
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().llm;
    assert_eq!(v.providers[0].models.len(), 1);
    let m = &v.providers[0].models[0];
    assert_eq!(m.max_output, None);
    assert_eq!(m.max_output_source, None);
    assert_eq!(m.supports_image, None);
    assert_eq!(
        m.input_modalities,
        vec!["text"],
        "无目录无钉死 = 纯文本缺省"
    );
    assert!(m.reasoning_ladder.is_empty());
    // 旧条目带 contextWindow = 落盘值即钉死（user）；输出/模态无钉死
    assert_eq!(m.sources.window.as_deref(), Some("user"));
    assert_eq!(m.sources.max_output, None);
    assert_eq!(m.sources.modalities, None);

    // 钉死写入 → 落盘 → 重开读回（sources 标记 user）
    store
        .update_llm_config(|cfg| {
            cfg.providers[0].models[0].max_output = Some(393216);
            cfg.providers[0].models[0].max_output_source = Some("user".into());
            cfg.providers[0].models[0].supports_image = Some(true);
            Ok(())
        })
        .unwrap();
    let again = SettingsStore::open(&root).unwrap();
    let m = &again.view().llm.providers[0].models[0];
    assert_eq!(m.max_output, Some(393216));
    assert_eq!(m.supports_image, Some(true));
    assert_eq!(
        m.input_modalities,
        vec!["text", "image"],
        "钉死覆盖展示模态"
    );
    assert_eq!(m.sources.max_output.as_deref(), Some("user"));
    assert_eq!(m.sources.modalities.as_deref(), Some("user"));
    std::fs::remove_dir_all(&root).ok();
}

/// 合成投影：钉死 ∪ 目录（钉死优先）；目录命中带梯子/价格/工具标记。
#[test]
fn synthesize_view_pins_win_over_catalog() {
    use crate::llm::catalog::{CatalogModel, CatalogPricing};
    let hit = CatalogModel {
        provider_key: "deepseek".into(),
        model_id: "deepseek-flash".into(),
        display_name: Some("DeepSeek V4.1 Flash".into()),
        context_window: Some(1_000_000),
        max_output: Some(393_216),
        input_modalities: vec!["text".into(), "image".into()],
        reasoning_options: vec!["disabled".into(), "low".into(), "high".into(), "max".into()],
        tool_call: Some(true),
        pricing: Some(CatalogPricing {
            input: Some(0.15),
            output: Some(0.6),
            cache_read: Some(0.003),
        }),
        family: Some("deepseek".into()),
    };
    // 无钉死 = 目录全量跟随（sources.catalog）
    let pm = ProviderModel {
        id: "deepseek-flash".into(),
        ..Default::default()
    };
    let v = synthesize_view(&pm, Some(&hit));
    assert_eq!(v.context_window, Some(1_000_000));
    assert_eq!(v.max_output, Some(393_216));
    assert_eq!(v.reasoning_ladder.len(), 4);
    assert_eq!(v.tool_call, Some(true));
    assert_eq!(v.pricing.unwrap().input, Some(0.15));
    assert_eq!(v.sources.window.as_deref(), Some("catalog"));
    assert_eq!(v.sources.reasoning.as_deref(), Some("catalog"));

    // 钉死覆盖：窗口/输出/模态 user 值赢，目录仍供梯子
    let pinned = ProviderModel {
        id: "deepseek-flash".into(),
        context_window: Some(131_072),
        max_output: Some(8_192),
        max_output_source: Some("user".into()),
        supports_image: Some(false),
        ..Default::default()
    };
    let v = synthesize_view(&pinned, Some(&hit));
    assert_eq!(v.context_window, Some(131_072));
    assert_eq!(v.max_output, Some(8_192));
    assert_eq!(v.input_modalities, vec!["text"], "钉死关图 = 纯文本");
    assert_eq!(v.sources.window.as_deref(), Some("user"));
    assert_eq!(v.sources.modalities.as_deref(), Some("user"));

    // 目录未命中 = 能力未知如实（模态纯文本缺省、梯子空、无价格）
    let v = synthesize_view(&pm, None);
    assert_eq!(v.input_modalities, vec!["text"]);
    assert!(v.reasoning_ladder.is_empty());
    assert_eq!(v.pricing, None);
    assert_eq!(v.sources.window, None);
}

// ---- 自定义请求头校验（批 C §8.1） ----

#[test]
fn headers_validation_rejects_reserved_and_malformed() {
    use std::collections::BTreeMap;
    let ok = BTreeMap::from([("User-Agent".into(), "Locowrk/1.0".into())]);
    assert!(validate_headers(&ok).is_ok());
    // 保留键（不区分大小写）：鉴权/逐跳/受控头不可自定义
    for reserved in [
        "authorization",
        "X-API-Key",
        "Host",
        "Content-Type",
        "cookie",
    ] {
        let bad = BTreeMap::from([(reserved.to_string(), "x".into())]);
        assert!(validate_headers(&bad).is_err(), "{reserved} 应被拒收");
    }
    // 非法 token（冒号 / 空白 / 空）
    for bad_key in ["bad:key", "bad key", ""] {
        let bad = BTreeMap::from([(bad_key.to_string(), "x".into())]);
        assert!(validate_headers(&bad).is_err(), "键 {bad_key:?} 应被拒收");
    }
    // 值含控制字符
    let bad = BTreeMap::from([("x-trace".into(), "a\nb".into())]);
    assert!(validate_headers(&bad).is_err());
    // 条数超限
    let many: BTreeMap<String, String> =
        (0..=32).map(|i| (format!("x-h{i}"), "v".into())).collect();
    assert!(validate_headers(&many).is_err());
}
