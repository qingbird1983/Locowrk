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
            None,
            Some(LlmUpdate {
                provider: Some("ollama".into()),
                model: Some("llama3".into()),
                base_url: Some("http://localhost:11434".into()),
                api_key: Some("sk-test".into()),
            }),
            None,
            None,
            None,
        None, )
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
            Some(LlmUpdate {
                api_key: Some("sk-secret-value".into()),
                ..Default::default()
            }),
            None,
            None,
            None,
        None, )
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
                Some(LlmUpdate {
                    api_key: k.map(Into::into),
                    base_url: b.map(Into::into),
                    ..Default::default()
                }),
                None,
                None,
                None,
            None, )
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
            Some(LlmUpdate {
                provider: Some("gemini".into()),
                ..Default::default()
            }),
            None,
            None,
            None,
        None, )
        .is_err());
    assert!(store
        .update(Some("fr".into()), None, None, None, None, None, None, )
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
        .update(None, Some("Alt+Space".into()), None, None, None, None, None, )
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
        .update(None, Some("Not A Key!!".into()), None, None, None, None, None, )
        .is_err());
    assert!(store
        .update(None, Some("".into()), None, None, None, None, None, )
        .is_err());
    assert_eq!(store.view().shortcut, "Alt+Space");
    std::fs::remove_dir_all(&root).ok();
}

// ---- 上下文管理区（⑨，Context and Memory.md §3） ----

#[test]
fn context_defaults_and_legacy_json_compat() {
    let root = temp_root("context-defaults");
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().context;
    // 缺省：窗口未设置 / 阈值 30% / 分层保留
    assert_eq!(v.window_tokens, None);
    assert_eq!(v.threshold_pct, 30);
    assert_eq!(v.strategy, kernel_agent::CompactStrategy::Layered);
    assert_eq!(v.threshold_tokens, 9_600, "默认 = FALLBACK_WINDOW × 30%");
    assert_eq!(v.suggested_pct, None, "窗口未知不给建议值");
    // 旧 settings.json 无 context 字段 → 字段级缺省，不解析失败
    std::fs::write(root.join(FILE_NAME), r#"{"locale":"zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().context;
    assert_eq!(v.threshold_pct, 30);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn context_update_math_and_validation() {
    let root = temp_root("context-update");
    let store = SettingsStore::open(&root).unwrap();
    store
        .update(
            None,
            None,
            None,
            Some(ContextUpdate {
                window_tokens: Some(Some(128_000)),
                threshold_pct: Some(40),
                strategy: Some(kernel_agent::CompactStrategy::SlidingWindow),
            }),
            None,
            None,
        None, )
        .unwrap();
    let v = store.view().context;
    assert_eq!(v.window_tokens, Some(128_000));
    assert_eq!(v.threshold_pct, 40);
    assert_eq!(v.strategy, kernel_agent::CompactStrategy::SlidingWindow);
    assert_eq!(v.threshold_tokens, 51_200, "128k × 40%");
    assert_eq!(v.suggested_pct, Some(25), "大窗口建议 25%");
    // compact_config 桥接（ctx-compact 共享锁内容物）
    let cfg = store.compact_config();
    assert_eq!(cfg.threshold_tokens, 51_200);
    assert_eq!(cfg.strategy, kernel_agent::CompactStrategy::SlidingWindow);
    // windowTokens: null = 清除窗口 → 回落 32k 保守保护
    store
        .update(
            None,
            None,
            None,
            Some(ContextUpdate {
                window_tokens: Some(None),
                ..Default::default()
            }),
            None,
            None,
        None, )
        .unwrap();
    assert_eq!(store.view().context.window_tokens, None);
    assert_eq!(store.view().context.threshold_tokens, 32_000 * 40 / 100);
    // 校验：阈值出界 / 窗口荒谬值拒绝且不污染内存态
    assert!(store
        .update(
            None,
            None,
            None,
            Some(ContextUpdate {
                threshold_pct: Some(5),
                ..Default::default()
            }),
            None,
            None,
        None, )
        .is_err());
    assert!(store
        .update(
            None,
            None,
            None,
            Some(ContextUpdate {
                window_tokens: Some(Some(50)),
                ..Default::default()
            }),
            None,
            None,
        None, )
        .is_err());
    assert_eq!(store.view().context.threshold_pct, 40);
    // 磁盘往返
    let v = SettingsStore::open(&root).unwrap().view().context;
    assert_eq!(v.threshold_pct, 40);
    assert_eq!(v.strategy, kernel_agent::CompactStrategy::SlidingWindow);
    std::fs::remove_dir_all(&root).ok();
}

// ---- 记忆注入区（⑪，Context and Memory.md §6.2） ----

#[test]
fn memory_defaults_legacy_compat_and_validation() {
    let root = temp_root("memory");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省：开 / 注入 5 条
    let v = store.view().memory;
    assert!(v.enabled);
    assert_eq!(v.top_k, 5);
    // memory_inject_config 桥接（MemoryInject 共享锁内容物）
    let cfg = store.memory_inject_config();
    assert!(cfg.enabled);
    assert_eq!(cfg.top_k, 5);
    // 旧 settings.json 无 memory 字段 → 字段级缺省，不解析失败
    std::fs::write(root.join(FILE_NAME), r#"{"locale":"zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().memory;
    assert!(v.enabled);
    assert_eq!(v.top_k, 5);
    // 更新 + 磁盘往返
    store
        .update(
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                enabled: Some(false),
                top_k: Some(3),
            }),
            None,
        None, )
        .unwrap();
    assert!(!store.view().memory.enabled);
    assert_eq!(store.view().memory.top_k, 3);
    let cfg = store.memory_inject_config();
    assert!(!cfg.enabled);
    assert_eq!(cfg.top_k, 3);
    assert_eq!(
        SettingsStore::open(&root).unwrap().view().memory.top_k,
        3
    );
    // 部分更新：只给 top_k，enabled 保持
    store
        .update(
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                top_k: Some(8),
                ..Default::default()
            }),
            None,
        None, )
        .unwrap();
    assert!(!store.view().memory.enabled);
    assert_eq!(store.view().memory.top_k, 8);
    // 校验：top_k 出界拒绝且不污染内存态
    for bad in [0, 21] {
        assert!(store
            .update(
                None,
                None,
                None,
                None,
                Some(MemoryUpdate {
                    top_k: Some(bad),
                    ..Default::default()
                }),
                None,
            None, )
            .is_err());
    }
    assert_eq!(store.view().memory.top_k, 8);
    std::fs::remove_dir_all(&root).ok();
}

mod shell;
mod tools;
