//! 零行为变更判据（这一刀的等价证明，夹具在父簇 `mod.rs`）。

use std::sync::Arc;

use serde_json::json;

use super::{
    call_through, ctx, obs, observe, scope_of, two_layer_matches, FakeRetrieval, ReceiptSink,
};
use crate::recall_log::RecallLog;

// ---------------------------------------------------------------- 零行为变更

/// **等价判据**：套壳前后 `Ok` 的 payload 逐字节相同，工具身份三件（spec / plugin_version /
/// plugin_id）一件都没被壳改掉——`plugin_id` 尤其要紧，⑩ 使用统计的插件归属吃它，
/// 壳把它变成 `None` 就是"这个工具不再属于任何插件"。
#[tokio::test]
async fn wrapper_returns_identical_payload_and_identity() {
    let payload = two_layer_matches();
    let inner = FakeRetrieval::ok("memory_search", payload.clone()).arc();
    let (sink, _lg) = obs("identity");
    let wrapped = observe(Arc::clone(&inner), sink);

    // 身份：三件都还是内部工具自己的值
    assert_eq!(wrapped.spec().name, inner.spec().name);
    assert_eq!(wrapped.spec().description, inner.spec().description);
    assert_eq!(wrapped.spec().schema, inner.spec().schema);
    assert_eq!(wrapped.spec().side_effect, inner.spec().side_effect);
    assert_eq!(wrapped.spec().exposure, inner.spec().exposure);
    assert_eq!(wrapped.plugin_version(), inner.plugin_version());
    assert_eq!(wrapped.plugin_id(), Some("memory-sqlite"));

    let got = wrapped
        .call(&ctx(Some("s1")), json!({"query": "rust"}))
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_string(&got).unwrap(),
        serde_json::to_string(&payload).unwrap(),
        "payload 必须逐字节透传"
    );
}

/// `Err` 原样上抛且**不落行**：「检索失败」不是「检索到 0 条」，把失败记成零条会把
/// 空块率与命中率一起骗好。
#[tokio::test]
async fn failed_call_passes_error_through_and_writes_nothing() {
    let (sink, lg) = obs("err");
    let inner = FakeRetrieval::ok("memory_search", json!({}))
        .failing()
        .arc();
    let err = call_through(inner, sink, Some("s1"))
        .await
        .expect_err("失败必须照失败返回，不许被观测面吞成 Ok");
    assert_eq!(err.to_string(), "tool `memory_search` failed: 库不可用");
    assert!(lg.recent(10).unwrap().is_empty(), "失败的调用不该出回执");
}

/// 不在采集面的工具 ⇒ **原对象返回**（同一个 `Arc`，不加一层壳），且不落行。
/// 用指针比对而不是"行为等价"：多一层无谓的壳以后会被读成"所有工具都被观测过"。
#[tokio::test]
async fn unobserved_tool_is_returned_untouched() {
    let (sink, lg) = obs("untouched");
    let inner = FakeRetrieval::ok("fs_read", json!({"content": "x"})).arc();
    let before = Arc::as_ptr(&inner) as *const ();
    let after = Arc::as_ptr(&observe(Arc::clone(&inner), Arc::clone(&sink))) as *const ();
    assert_eq!(before, after, "未采集的工具不许被套壳");

    call_through(inner, sink, Some("s1")).await.unwrap();
    assert!(lg.recent(10).unwrap().is_empty());
}

/// **观测不许拖垮被观测者**（`recall_log/mod.rs` 不变量 2 的检索侧同族）：库写不进去
/// （父目录不存在）时调用仍返回原 payload，不抛、不 panic，也不自己把目录建出来。
#[tokio::test]
async fn unwritable_log_does_not_break_the_call() {
    let path = crate::testutil::unique_dir("recallreceipt-nope").join("usage.sqlite3");
    let sink = ReceiptSink::new(RecallLog::at(&path), scope_of());
    let payload = json!({"query": "q", "matches": []});
    let got = call_through(
        FakeRetrieval::ok("memory_search", payload.clone()).arc(),
        sink,
        Some("s1"),
    )
    .await
    .unwrap();
    assert_eq!(got, payload);
    assert!(
        !path.exists(),
        "写不进去时也不该把父目录凭空建出来（`RecallLog::at` 不碰盘的契约）"
    );
}
