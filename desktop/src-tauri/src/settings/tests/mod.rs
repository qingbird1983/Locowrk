//! settings 模块测试：默认值、落盘往返、秘钥不出 IPC、更新语义。

use super::*;

fn temp_root(tag: &str) -> PathBuf {
    let dir = crate::testutil::work_dir(&format!("settings-{}", tag));
    dir
}

#[test]
fn missing_file_yields_defaults() {
    let root = temp_root("defaults");
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view();
    assert_eq!(v.locale, LOCALE_SYSTEM);
    assert_eq!(v.llm.provider, "openai");
    assert_eq!(v.llm.model, "");
    assert!(!v.llm.api_key_set);
    assert_eq!(v.data_dir, root.display().to_string());
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn update_persists_and_roundtrips() {
    let root = temp_root("roundtrip");
    SettingsStore::open(&root)
        .unwrap()
        .update(
            Some("en".into()),
            None,
            None,
            Some(LlmUpdate {
                provider: Some("ollama".into()),
                model: Some("llama3".into()),
                base_url: Some("http://localhost:11434".into()),
                api_key: Some("sk-test".into()),
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
    // 重新打开：从磁盘读回，而非内存态
    let v = SettingsStore::open(&root).unwrap().view();
    assert_eq!(v.locale, "en");
    assert_eq!(v.llm.provider, "ollama");
    assert_eq!(v.llm.model, "llama3");
    assert!(v.llm.api_key_set);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn api_key_never_leaks_into_view() {
    let root = temp_root("no-leak");
    let store = SettingsStore::open(&root).unwrap();
    store
        .update(
            None,
            None,
            None,
            Some(LlmUpdate {
                api_key: Some("sk-secret-value".into()),
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
    let json = serde_json::to_string(&store.view()).unwrap();
    assert!(!json.contains("sk-secret-value"));
    assert!(json.contains("\"apiKeySet\":true"));
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn empty_string_clears_none_keeps() {
    let root = temp_root("clear");
    let store = SettingsStore::open(&root).unwrap();
    let set = |k: Option<&str>, b: Option<&str>| {
        store
            .update(
                None,
                None,
                None,
                Some(LlmUpdate {
                    api_key: k.map(Into::into),
                    base_url: b.map(Into::into),
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
            .unwrap()
    };
    set(Some("sk-a"), Some("http://x"));
    assert!(store.view().llm.api_key_set);
    // None = 不变；Some("") = 清除
    set(None, None);
    assert!(store.view().llm.api_key_set);
    set(Some(""), Some(""));
    assert!(!store.view().llm.api_key_set);
    assert_eq!(store.view().llm.base_url, None);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn rejects_unknown_provider_and_locale() {
    let root = temp_root("reject");
    let store = SettingsStore::open(&root).unwrap();
    assert!(store
        .update(
            None,
            None,
            None,
            Some(LlmUpdate {
                provider: Some("gemini".into()),
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
    assert!(store
        .update(
            Some("fr".into()),
            None,
            None,
            None,
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
    // 校验失败不得污染内存态
    assert_eq!(store.view().locale, LOCALE_SYSTEM);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn shortcut_default_update_and_validation() {
    let root = temp_root("shortcut");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省键位进投影（tray.rs 启动注册读这里）
    assert_eq!(store.view().shortcut, SHORTCUT_DEFAULT);
    // 合法键位更新 + 磁盘往返
    store
        .update(
            None,
            None,
            Some("Alt+Space".into()),
            None,
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
    assert_eq!(store.view().shortcut, "Alt+Space");
    assert_eq!(
        SettingsStore::open(&root).unwrap().view().shortcut,
        "Alt+Space"
    );
    // 旧 settings.json 无 shortcut 字段 → 字段级缺省（不是空串）
    std::fs::write(root.join(FILE_NAME), r#"{"locale":"zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view();
    assert_eq!(v.locale, "zh-CN");
    assert_eq!(v.shortcut, SHORTCUT_DEFAULT);
    // 非法键位 / 空串：拒绝且不污染内存态
    assert!(store
        .update(
            None,
            None,
            Some("Not A Key!!".into()),
            None,
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
    assert!(store
        .update(
            Some("".into()),
            None,
            None,
            None,
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
    assert_eq!(store.view().shortcut, "Alt+Space");
    std::fs::remove_dir_all(&root).ok();
}

mod appearance;
mod chat;
mod fact_cards;
mod instruction_budget;
mod llm;
mod llm_profile;
mod llm_provider;
mod memory;
mod network_proxy;
mod window;

mod developer;
mod tools;
