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
