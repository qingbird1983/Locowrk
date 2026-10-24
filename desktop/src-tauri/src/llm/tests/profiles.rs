//! T25 下沉后的档位装配行为钉子（`profiles::save_profile`）：纯 `&mut LlmConfig`
//! 面，不需要 State（此前命令本体要 Tauri `State`，测试打不进去）。**不测**
//! api_key 非空路径——那会真写系统凭据库；None（保持）/ 校验拒绝 / 装配形状
//! 这些纯配置路径全覆盖。

use crate::llm::profiles::save_profile;
use crate::settings::{LlmConfig, ProviderInput, ProviderModel};

fn input(id: Option<&str>, label: &str, kind: &str) -> ProviderInput {
    ProviderInput {
        id: id.map(str::to_string),
        label: label.to_string(),
        kind: kind.to_string(),
        base_url: None,
        api_key: None,
        model: "m1".to_string(),
        window_tokens: None,
        enabled: None,
        models: None,
        headers: None,
    }
}

#[test]
fn new_profile_gets_id_and_active_pointer() {
    let mut cfg = LlmConfig::default();
    save_profile(&mut cfg, &input(None, "档A", "openai")).expect("新建应成功");
    assert_eq!(cfg.providers.len(), 1);
    let p = &cfg.providers[0];
    assert_eq!(p.label, "档A");
    assert!(p.enabled, "缺省开");
    assert_eq!(cfg.active, p.id, "空指针回落首档");
}

#[test]
fn update_keeps_fields_not_sent() {
    let mut cfg = LlmConfig::default();
    save_profile(&mut cfg, &input(None, "档A", "openai")).expect("新建应成功");
    let id = cfg.providers[0].id.clone();
    cfg.providers[0].api_key = Some("sk-old".to_string());
    cfg.providers[0].enabled = false;
    // api_key None = 保持已存值；enabled None = 保持已存档
    save_profile(&mut cfg, &input(Some(&id), "档A2", "openai")).expect("更新应成功");
    let p = &cfg.providers[0];
    assert_eq!(p.id, id, "更新不换 id");
    assert_eq!(p.label, "档A2");
    assert_eq!(p.api_key.as_deref(), Some("sk-old"));
    assert!(!p.enabled);
}

#[test]
fn validation_rejects_and_does_not_touch_config() {
    let mut cfg = LlmConfig::default();
    assert_eq!(
        save_profile(&mut cfg, &input(None, "  ", "openai")),
        Err("档位名称不能为空".to_string())
    );
    assert_eq!(
        save_profile(&mut cfg, &input(None, "档A", "nope")),
        Err("不支持的 provider: nope（可选 openai / anthropic / ollama）".to_string())
    );
    assert_eq!(
        save_profile(&mut cfg, &input(Some("ghost"), "档A", "openai")),
        Err("档位不存在: ghost".to_string())
    );
    assert!(cfg.providers.is_empty(), "拒绝路径不改配置");
}

#[test]
fn models_list_dedups_and_falls_back_to_first() {
    let mut cfg = LlmConfig::default();
    let mut i = input(None, "档A", "openai");
    i.model = "M3".to_string();
    i.models = Some(vec![
        ProviderModel {
            id: " M1 ".to_string(),
            ..Default::default()
        },
        ProviderModel {
            id: "m1".to_string(),
            ..Default::default()
        },
        ProviderModel {
            id: String::new(),
            ..Default::default()
        },
        ProviderModel {
            id: "M2".to_string(),
            max_output: Some(4096),
            ..Default::default()
        },
    ]);
    save_profile(&mut cfg, &i).expect("保存应成功");
    let p = &cfg.providers[0];
    let ids: Vec<&str> = p.models.iter().map(|m| m.id.as_str()).collect();
    assert_eq!(ids, vec!["M1", "M2"], "trim + 小写去重 + 保序 + 空串剔除");
    assert_eq!(
        p.models[1].max_output_source.as_deref(),
        Some("user"),
        "钉死标记只随用户写入"
    );
    assert_eq!(p.model, "M1", "原 model 不在清单 = 改指首项");
}

#[test]
fn existing_profile_is_replaced_in_place() {
    let mut cfg = LlmConfig::default();
    save_profile(&mut cfg, &input(None, "档A", "openai")).expect("新建应成功");
    let id = cfg.providers[0].id.clone();
    let before = serde_json::to_string(&cfg.providers).unwrap();
    let mut i = input(Some(&id), "档A", "openai");
    i.base_url = Some("  ".to_string());
    save_profile(&mut cfg, &i).expect("再存应成功");
    assert_eq!(
        serde_json::to_string(&cfg.providers).unwrap(),
        before,
        "空 base_url 过滤成 None 后与已存档逐字节相同（无重复条目）"
    );
    assert_eq!(cfg.providers.len(), 1);
}
