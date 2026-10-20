//! 思考语言注入测试：解析矩阵（纯函数）+ 会话锁死语义（真实
//! SettingsStore 落盘往返）+ 追加形状。

use std::sync::Arc;

use cordis_core::{WaterfallDecision, WaterfallListener};
use kernel_agent::{points, RequestPayload};
use serde_json::{json, Value};

use super::{resolve, ThinkingLanguageInject};
use crate::settings::{SettingsStore, ThinkingUpdate};

fn temp_root(tag: &str) -> std::path::PathBuf {
    let dir = crate::testutil::work_dir(&format!("lw-thinking-{}", tag));
    dir
}

/// 打开临时设置库并把 locale / thinking 一次写死（显式 locale 避免
/// follow_ui + system 档走到 OS 语言解析——测试要跨机器确定）。
fn store_with(root: &std::path::Path, locale: &str, language: &str) -> SettingsStore {
    let store = SettingsStore::open(root).unwrap();
    store
        .update(
            Some(locale.into()),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(ThinkingUpdate {
                language: language.into(),
            }),
            None,
        )
        .unwrap();
    store
}

/// 跑一环，返回**最终 payload**（并断言收束 = `Continue`）。
///
/// 台账 T2 之前这一环返回 `Replace(整包)`，测试要从决策里取载荷；现在改写就地
/// 发生，决策只剩"放行"这一种，结果必须从 payload 读。调用点形状
/// `system_of(handle_request(..).await)` 保持不变。
async fn handle_request(
    listener: &ThinkingLanguageInject,
    session: Option<&str>,
    system: Option<&str>,
) -> Value {
    let mut payload = json!({ "messages": [] });
    if let Some(s) = session {
        payload["session_id"] = json!(s);
    }
    if let Some(sys) = system {
        payload["system"] = json!(sys);
    }
    let d = listener.handle(points::REQUEST, &mut payload).await;
    assert!(
        matches!(d, WaterfallDecision::Continue),
        "就地改写后应 Continue，实际 {d:?}"
    );
    payload
}

fn system_of(payload: Value) -> Option<String> {
    serde_json::from_value::<RequestPayload>(payload)
        .unwrap()
        .system
}

#[test]
fn resolve_matrix() {
    // 显式档：与界面 / OS 语言无关
    assert_eq!(resolve("zh-CN", "en", "en-US"), super::ZH_HINT);
    assert_eq!(resolve("en-US", "zh-CN", "zh-CN"), super::EN_HINT);
    assert_eq!(resolve("none", "zh-CN", "zh-CN"), "");
    // 跟随界面：显式 locale 直接映射
    assert_eq!(resolve("follow_ui", "zh-CN", "en-US"), super::ZH_HINT);
    assert_eq!(resolve("follow_ui", "en", "zh-CN"), super::EN_HINT);
    // 跟随界面 + system：OS 语言解析（zh* → 中文，其余 → 英文）
    assert_eq!(resolve("follow_ui", "system", "zh-TW"), super::ZH_HINT);
    assert_eq!(resolve("follow_ui", "system", "en-US"), super::EN_HINT);
    assert_eq!(resolve("follow_ui", "system", "fr"), super::EN_HINT);
    // 未知档值按缺省档语义回落跟随界面（fail-open）
    assert_eq!(resolve("bogus", "zh-CN", "en"), super::ZH_HINT);
}

#[tokio::test]
async fn appends_to_system_tail() {
    let root = temp_root("append");
    let listener = ThinkingLanguageInject::new(Arc::new(store_with(&root, "zh-CN", "zh-CN")));

    // 无 system：hint 成为 system
    let sys = system_of(handle_request(&listener, Some("s1"), None).await).unwrap();
    assert_eq!(sys, super::ZH_HINT);
    // 已有 system：\n\n 追加在尾（TreeDirectoryInject 同款形状）
    let sys = system_of(handle_request(&listener, Some("s1"), Some("BASE")).await).unwrap();
    assert_eq!(sys, format!("BASE\n\n{}", super::ZH_HINT));
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn none_mode_touches_nothing() {
    let root = temp_root("none");
    let listener = ThinkingLanguageInject::new(Arc::new(store_with(&root, "zh-CN", "none")));

    assert_eq!(
        system_of(handle_request(&listener, Some("s1"), None).await).as_deref(),
        None::<&str>,
        "none 档：system 一个字节都没加"
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn hint_locks_per_session_across_setting_change() {
    let root = temp_root("lock");
    let store = Arc::new(store_with(&root, "en", "en-US"));
    let listener = ThinkingLanguageInject::new(Arc::clone(&store));

    // 首请求解析并锁死
    let sys = system_of(handle_request(&listener, Some("s1"), None).await).unwrap();
    assert_eq!(sys, super::EN_HINT);

    // 中途改设置（文档 §8）：s1 不变（前缀缓存红线），新会话 s2 用新值
    store
        .update(
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
            Some(ThinkingUpdate {
                language: "zh-CN".into(),
            }),
            None,
        )
        .unwrap();
    let sys = system_of(handle_request(&listener, Some("s1"), None).await).unwrap();
    assert_eq!(sys, super::EN_HINT, "已开会话锁死");
    let sys = system_of(handle_request(&listener, Some("s2"), None).await).unwrap();
    assert_eq!(sys, super::ZH_HINT, "新会话用新配置");
    let _ = std::fs::remove_dir_all(&root);
}

#[tokio::test]
async fn non_request_key_ignored() {
    let root = temp_root("key");
    let listener = ThinkingLanguageInject::new(Arc::new(store_with(&root, "en", "en-US")));
    let mut v = json!({});
    assert_eq!(
        listener.handle("agent/other", &mut v).await,
        WaterfallDecision::Continue
    );
    assert_eq!(
        v,
        json!({}),
        "非 request 点：载荷一字未动（守卫根本没建立）"
    );
    let _ = std::fs::remove_dir_all(&root);
}
