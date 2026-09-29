//! settings 模块测试：默认值、落盘往返、秘钥不出 IPC、更新语义。

use super::*;

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("locowrk-settings-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
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
            Some(LlmUpdate {
                provider: Some("ollama".into()),
                model: Some("llama3".into()),
                base_url: Some("http://localhost:11434".into()),
                api_key: Some("sk-test".into()),
            }),
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
            Some(LlmUpdate {
                api_key: Some("sk-secret-value".into()),
                ..Default::default()
            }),
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
                Some(LlmUpdate {
                    api_key: k.map(Into::into),
                    base_url: b.map(Into::into),
                    ..Default::default()
                }),
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
            Some(LlmUpdate {
                provider: Some("gemini".into()),
                ..Default::default()
            })
        )
        .is_err());
    assert!(store.update(Some("fr".into()), None).is_err());
    // 校验失败不得污染内存态
    assert_eq!(store.view().locale, LOCALE_SYSTEM);
    std::fs::remove_dir_all(&root).ok();
}
