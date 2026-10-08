//! LLM 档位制测试（v2.58，DESIGN §13.1）：旧平面迁移 / 生效投影 /
//! 平面更新物化缺省档 / 切档校验 / 档位命令层语义（save/delete）。

use super::*;

// ---- 旧平面 llm 区迁移（open 读入，幂等） ----

#[test]
fn legacy_flat_llm_migrates_to_default_profile() {
    let root = temp_root("llm-migrate");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join(FILE_NAME),
        r#"{"llm":{"provider":"deepseek","model":"deepseek-flash","baseUrl":"https://api.deepseek.com","apiKey":"sk-old","windowTokens":131072}}"#,
    )
    .unwrap();
    let store = SettingsStore::open(&root).unwrap();
    // 生效投影 = 档位原值（平面字段语义不变）
    let v = store.view().llm;
    assert_eq!(v.active, "default");
    assert_eq!(v.providers.len(), 1);
    assert_eq!(v.provider, "deepseek");
    assert_eq!(v.model, "deepseek-flash");
    assert_eq!(v.base_url.as_deref(), Some("https://api.deepseek.com"));
    assert!(v.api_key_set);
    assert_eq!(v.window_tokens, Some(131072));
    // 旧档位无 enabled 字段 = 启用（字段级 serde 缺省；布尔缺省 false 会
    // 把存量档全变停用——30c 走查实抓后修）
    assert!(v.providers[0].enabled);
    // 后端侧（含 key 原值）同源
    assert_eq!(store.llm().api_key.as_deref(), Some("sk-old"));
    // 幂等：再次 open 不二次包裹
    let again = SettingsStore::open(&root).unwrap();
    assert_eq!(again.view().llm.providers.len(), 1);
    std::fs::remove_dir_all(&root).ok();
}

// ---- 生效投影与平面更新 ----

#[test]
fn flat_update_materializes_default_profile() {
    let root = temp_root("llm-materialize");
    let store = SettingsStore::open(&root).unwrap();
    // 出厂空表：平面更新按需物化 default 档（旧单档语义）
    assert!(store.view().llm.providers.is_empty());
    store
        .update(
            None,
            None,
            None,
            Some(LlmUpdate {
                provider: Some("ollama".into()),
                model: Some("qwen3:8b".into()),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    let v = store.view().llm;
    assert_eq!(v.active, "default");
    assert_eq!(v.provider, "ollama");
    assert_eq!(v.model, "qwen3:8b");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn active_switch_rejects_unknown_id_and_applies_to_target() {
    let root = temp_root("llm-active");
    let store = SettingsStore::open(&root).unwrap();
    // 物化 default 档
    store
        .update(
            None,
            None,
            None,
            Some(LlmUpdate {
                model: Some("m1".into()),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    // 不存在的档位 id 拒绝且不污染内存态
    assert!(store
        .update(
            None,
            None,
            None,
            Some(LlmUpdate {
                active: Some("nope".into()),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .is_err());
    assert_eq!(store.view().llm.active, "default");
    std::fs::remove_dir_all(&root).ok();
}

// ---- 档位命令层语义（update_llm_config 的 apply 闭包与命令层同逻辑） ----

#[test]
fn profile_save_delete_semantics() {
    let root = temp_root("llm-profiles");
    let store = SettingsStore::open(&root).unwrap();

    // 增两档（命令层闭包同形：新档 id 生成、api_key 缺省 None）
    let mk = |label: &str, kind: &str, model: &str| ProviderInput {
        id: None,
        label: label.into(),
        kind: kind.into(),
        base_url: None,
        api_key: None,
        model: model.into(),
        window_tokens: None,
        enabled: None,
        models: None,
        headers: None,
    };
    let cfg = store
        .update_llm_config(|cfg| {
            for input in [
                mk("DeepSeek", "openai", "deepseek-flash"),
                mk("本地", "ollama", "qwen3:8b"),
            ] {
                let id = new_profile_id();
                cfg.providers.push(ProviderProfile {
                    id,
                    label: input.label,
                    kind: input.kind,
                    base_url: None,
                    api_key: None,
                    model: input.model,
                    window_tokens: None,
                    enabled: true,
                    models: Vec::new(),
                    ..Default::default()
                });
            }
            cfg.active = cfg.providers[0].id.clone();
            Ok(())
        })
        .unwrap();
    assert_eq!(cfg.providers.len(), 2);

    // 切活动档（平面更新走 active）
    let second = cfg.providers[1].id.clone();
    store
        .update(
            None,
            None,
            None,
            Some(LlmUpdate {
                active: Some(second.clone()),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(store.view().llm.active, second);
    assert_eq!(store.view().llm.model, "qwen3:8b", "生效投影随活动档走");

    // 删活动档 = 指针落首档
    let first = cfg.providers[0].id.clone();
    let cfg = store
        .update_llm_config(|cfg| {
            cfg.providers.retain(|p| p.id != second);
            if cfg.active == second {
                cfg.active = cfg
                    .providers
                    .first()
                    .map(|p| p.id.clone())
                    .unwrap_or_default();
            }
            Ok(())
        })
        .unwrap();
    assert_eq!(cfg.active, first);
    assert_eq!(store.view().llm.model, "deepseek-flash");
    std::fs::remove_dir_all(&root).ok();
}

// ---- ㉚ 模型配置页：models 整表替换 / 默认模型改指 / 停用档回落跳过 ----

#[test]
fn models_replace_repoints_model_and_disabled_skips_active() {
    let root = temp_root("llm-models");
    let store = SettingsStore::open(&root).unwrap();
    let pm = crate::settings::ProviderModel {
        id: "m2-x".into(),
        alias: None,
        context_window: Some(262144),
        ..Default::default()
    };
    let cfg = store
        .update_llm_config(|cfg| {
            cfg.providers.push(ProviderProfile {
                id: "a".into(),
                label: "A".into(),
                kind: "openai".into(),
                model: "m1".into(),
                models: vec![crate::settings::ProviderModel {
                    id: "m1".into(),
                    alias: None,
                    context_window: None,
                    ..Default::default()
                }],
                enabled: true,
                ..Default::default()
            });
            cfg.providers.push(ProviderProfile {
                id: "b".into(),
                label: "B".into(),
                kind: "ollama".into(),
                model: "q3".into(),
                enabled: true,
                ..Default::default()
            });
            cfg.active = "a".into();
            Ok(())
        })
        .unwrap();
    assert_eq!(cfg.active_profile().map(|p| p.id.as_str()), Some("a"));

    // 停用活动档：回落跳到未停用档（b），平面投影随 b 走
    let cfg = store
        .update_llm_config(|cfg| {
            cfg.providers[0].enabled = false;
            Ok(())
        })
        .unwrap();
    assert_eq!(cfg.active_profile().map(|p| p.id.as_str()), Some("b"));
    assert_eq!(store.view().llm.model, "q3");
    assert_eq!(cfg.active, "a", "指针不动，只是回落跳过停用档");

    // models 整表替换：原 model 不在清单 → 改指首项（save 语义）
    store
        .update_llm_config(|cfg| {
            cfg.providers[0].models = vec![pm.clone()];
            if !cfg.providers[0]
                .models
                .iter()
                .any(|m| m.id == cfg.providers[0].model)
            {
                cfg.providers[0].model = cfg.providers[0].models[0].id.clone();
            }
            Ok(())
        })
        .unwrap();
    let v = store.view().llm;
    let b = v.providers.iter().find(|p| p.id == "a").unwrap();
    assert_eq!(b.models.len(), 1);
    assert_eq!(b.models[0].context_window, Some(262144));
    std::fs::remove_dir_all(&root).ok();
}

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

// ---- 网络代理（批 C §8.5） ----

#[test]
fn network_proxy_validation_and_normalization() {
    assert!(NetworkProxy {
        mode: "system".into(),
        url: None
    }
    .validate()
    .is_ok());
    assert!(NetworkProxy {
        mode: "direct".into(),
        url: None
    }
    .validate()
    .is_ok());
    let custom = NetworkProxy {
        mode: "custom".into(),
        url: Some(" http://127.0.0.1:7890 ".into()),
    };
    assert!(custom.validate().is_ok());
    // custom 缺 url / 非 http 形态拒绝
    assert!(NetworkProxy {
        mode: "custom".into(),
        url: None
    }
    .validate()
    .is_err());
    assert!(NetworkProxy {
        mode: "custom".into(),
        url: Some("socks5://x".into())
    }
    .validate()
    .is_err());
    // 未知模式拒绝
    assert!(NetworkProxy {
        mode: "proxy".into(),
        url: None
    }
    .validate()
    .is_err());
    // 归一：system/direct 不带 url；custom 去 blanks
    assert_eq!(
        NetworkProxy {
            mode: "direct".into(),
            url: Some("http://x".into())
        }
        .normalized()
        .url,
        None
    );
    assert_eq!(
        NetworkProxy {
            mode: "custom".into(),
            url: Some(" http://a:1 ".into())
        }
        .normalized()
        .url
        .as_deref(),
        Some("http://a:1")
    );
}
