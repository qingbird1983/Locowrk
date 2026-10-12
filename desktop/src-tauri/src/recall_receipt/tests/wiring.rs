//! 接线承重判据（生产出口 `resolve` 那一段，夹具在父簇 `mod.rs`）。

use std::sync::Arc;

use kernel_agent::ToolSource;
use plugin_api::Tool;
use plugin_host::{PluginRegistry, RegistryToolSource};
use serde_json::json;

use super::{ctx, obs, obs_at, observe, read_hit, two_layer_matches, FakeRetrieval, ReceiptSink};
use crate::tool_source::KernelToolSource;

// ---------------------------------------------------------------- 接线承重

fn catalog_with(builtins: Vec<Arc<dyn Tool>>, sink: Option<Arc<ReceiptSink>>) -> KernelToolSource {
    let mut src = KernelToolSource::new(RegistryToolSource::new(Arc::new(PluginRegistry::new())));
    for b in builtins {
        src = src.with_builtin(b);
    }
    if let Some(sink) = sink {
        src = src.with_receipt_sink(sink);
    }
    src
}

/// **接线的承重证明**：走生产出口 `KernelToolSource::resolve` 拿到工具再调用，账要落进去，
/// 且只有采集面出账。两件在同一棵树上对判，否则"测住了装饰器、没测住谁用它"就是空转。
#[tokio::test]
async fn resolve_wraps_only_the_observed_tools() {
    let (sink, lg) = obs("wired");
    let src = catalog_with(
        vec![
            FakeRetrieval::ok("memory_search", two_layer_matches()).arc(),
            FakeRetrieval::ok("fs_read", json!({"content": "x"})).arc(),
        ],
        Some(sink),
    );

    // 采集面：经 resolve 出来的工具调用后出账，会话键与键位都取自执行期 CallCtx
    let tool = src.resolve(Some("sess-9"), "memory_search").await.unwrap();
    tool.call(&ctx(Some("sess-9")), json!({"query": "rust"}))
        .await
        .unwrap();
    let rows = lg.recent(10).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].mode, "memory_search");
    assert_eq!(rows[0].session_key, "sess-9");
    assert_eq!(rows[0].scope_id, "F:/ws/a");

    // 非采集面：同一棵树下 fs_read 不出账
    src.resolve(Some("sess-9"), "fs_read")
        .await
        .unwrap()
        .call(&ctx(Some("sess-9")), json!({"path": "a"}))
        .await
        .unwrap();
    let rows = lg.recent(10).unwrap();
    assert_eq!(rows.len(), 1, "只有采集面的三个工具出账");
    assert_eq!(rows[0].mode, "memory_search");
}

/// 未接采集件 = 完全不观测，也**不落一个字节**（`with_receipt_sink` 是唯一开关；
/// kernel-bin 与历批测试路径同形）。这条挡的是"默认开启 / 忘关"的形态漂移。
#[tokio::test]
async fn unwired_source_records_nothing_and_touches_no_disk() {
    let (_sink, path, lg) = obs_at("unwired");
    let src = catalog_with(
        vec![FakeRetrieval::ok("memory_search", two_layer_matches()).arc()],
        None,
    );
    src.resolve(Some("s1"), "memory_search")
        .await
        .unwrap()
        .call(&ctx(Some("s1")), json!({"query": "rust"}))
        .await
        .unwrap();
    // ⚠ 顺序是判据的一部分：`recent()` 自己会 open 连接并跑 DDL，**先把文件建出来**。
    // 先读表再判存在 = 判据自己造出证据（首跑就是这么红的，不是产品缺陷）。
    assert!(!path.exists(), "未接线的树不许自己把观测库建出来");
    assert!(lg.recent(10).unwrap().is_empty());
}

/// 无插件身份的内置工具（`plugin_id() = None`）也要照原样透传——采集壳不许把
/// `None` 变成 `Some` 或反过来（`memory_project_history` 就是宿主内置，本就无插件身份）。
#[tokio::test]
async fn builtin_without_plugin_id_keeps_none() {
    let (sink, lg) = obs("noid");
    let inner = FakeRetrieval::ok("memory_read", read_hit("k", "global"))
        .without_plugin()
        .arc();
    let wrapped = observe(inner, sink);
    assert_eq!(wrapped.plugin_id(), None);
    wrapped
        .call(&ctx(Some("s1")), json!({"key": "k"}))
        .await
        .unwrap();
    assert_eq!(lg.recent(10).unwrap()[0].mode, "memory_read");
}
