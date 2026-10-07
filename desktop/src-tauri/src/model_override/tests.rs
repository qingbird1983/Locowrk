//! ModelOverrideStore 测试：写穿落盘、缺字段回落、清除还原。

use super::ModelOverrideStore;
use std::fs;
use std::path::PathBuf;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "locowrk-model-override-{tag}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn set_writes_through_and_get_caches() {
    let dir = temp_dir("write");
    let store = ModelOverrideStore::new(dir.clone());

    // 未设置 = None（跟随设置缺省）
    assert_eq!(store.get("s1"), None);

    // 写穿后旁车可读（重启语义：新 store 从盘读回）
    store
        .set("s1", Some(("p-deepseek", "deepseek-flash")))
        .unwrap();
    assert_eq!(
        store.get("s1"),
        Some(("p-deepseek".into(), "deepseek-flash".into()))
    );
    let reopened = ModelOverrideStore::new(dir.clone());
    assert_eq!(
        reopened.get("s1"),
        Some(("p-deepseek".into(), "deepseek-flash".into()))
    );

    // 清除 = 双字段同清，旁车字段消失（新 store 模拟重启——旧 store 的
    // 缓存命中不代表盘上还有）
    store.set("s1", None).unwrap();
    assert_eq!(store.get("s1"), None);
    let after_clear = ModelOverrideStore::new(dir.clone());
    assert_eq!(after_clear.get("s1"), None);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn partial_sidecar_fields_are_ignored() {
    let dir = temp_dir("partial");
    // 手改旁车只写 model 不写 provider（旧数据 / 损坏）：成对校验兜住
    let meta = dir.join("s2.meta.json");
    fs::write(&meta, r#"{"model":"deepseek-flash"}"#).unwrap();
    let store = ModelOverrideStore::new(dir.clone());
    assert_eq!(store.get("s2"), None);
    fs::remove_dir_all(&dir).ok();
}

#[test]
fn thinking_level_roundtrips_and_clears_independently() {
    let dir = temp_dir("thinking");
    let store = ModelOverrideStore::new(dir.clone());
    assert_eq!(store.get_thinking("s3"), None);
    store.set_thinking("s3", Some("high")).unwrap();
    assert_eq!(store.get_thinking("s3"), Some("high".into()));
    // 重启（新 store）读回
    let reopened = ModelOverrideStore::new(dir.clone());
    assert_eq!(reopened.get_thinking("s3"), Some("high".into()));
    // 与模型覆盖互不干扰
    store.set("s3", Some(("p", "m"))).unwrap();
    assert_eq!(store.get_thinking("s3"), Some("high".into()));
    // 空串 = 清除
    store.set_thinking("s3", Some("")).unwrap();
    assert_eq!(store.get_thinking("s3"), None);
    // 旁车里仍有 model 字段（合并写保留）
    let reopened = ModelOverrideStore::new(dir.clone());
    assert_eq!(reopened.get("s3"), Some(("p".into(), "m".into())));
    assert_eq!(reopened.get_thinking("s3"), None);
    fs::remove_dir_all(&dir).ok();
}
