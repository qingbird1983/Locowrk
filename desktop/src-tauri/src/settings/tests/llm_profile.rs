//! 档位命令层测试（自 `llm.rs` 批 F-13 拆出，用例本体逐字未改）：平面更新物化缺省档
//! / 切档校验 / 档位增删语义 / 模型清单替换与停用跳过 / 子代理钉档校验与清除。

use super::*;

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

/// 批 F 子代理钉档：合法钉档落盘 / 空 provider_id = 清除 / 指向不存在
/// 档位拒收不污染内存态（v2.65「空串 = 清除」约定同款）。
#[test]
fn subagent_pin_update_validates_and_clears() {
    use crate::settings::{LlmConfig, LlmUpdate, ProviderProfile, SubagentPin};
    let mut cfg = LlmConfig {
        active: "p1".into(),
        providers: vec![ProviderProfile {
            id: "p1".into(),
            label: "DeepSeek".into(),
            kind: "openai".into(),
            model: "deepseek-flash".into(),
            enabled: true,
            ..Default::default()
        }],
        ..Default::default()
    };
    // 合法钉档
    cfg.apply_update(&LlmUpdate {
        subagent: Some(SubagentPin {
            provider_id: "p1".into(),
            model: "deepseek-v4-pro".into(),
            thinking_level: Some("high".into()),
        }),
        ..Default::default()
    })
    .unwrap();
    let pin = cfg.subagent.clone().expect("pin set");
    assert_eq!(pin.model, "deepseek-v4-pro");
    assert_eq!(pin.thinking_level.as_deref(), Some("high"));
    // 空 provider_id = 清除
    cfg.apply_update(&LlmUpdate {
        subagent: Some(SubagentPin::default()),
        ..Default::default()
    })
    .unwrap();
    assert!(cfg.subagent.is_none(), "空钉档 = 清除");
    // 指向不存在档位：Err 且不污染
    let err = cfg
        .apply_update(&LlmUpdate {
            subagent: Some(SubagentPin {
                provider_id: "ghost".into(),
                model: "m".into(),
                thinking_level: None,
            }),
            ..Default::default()
        })
        .unwrap_err();
    assert!(err.contains("不存在"));
    assert!(cfg.subagent.is_none());
    // 停用档拒收
    cfg.providers[0].enabled = false;
    let err = cfg
        .apply_update(&LlmUpdate {
            subagent: Some(SubagentPin {
                provider_id: "p1".into(),
                model: "m".into(),
                thinking_level: None,
            }),
            ..Default::default()
        })
        .unwrap_err();
    assert!(err.contains("停用"));
}
