//! 端到端集成测试：阶段八⑧ WASM 加载器（§5.8，真实 wasmtime 实例化）。
//!
//! - wasm-ping：WAT 客户机落成 `.wasm` 工件 → registry 按 `.wasm` 分发
//!   → 工具目录 → 调用（含 log 导入与静态输出）
//! - native 与 wasm 混载同一 registry：工具目录共存、各自可解析调用
//!
//! native 侧沿用 tools_batch2 模式：测试自建产物暂存到
//! `target/locowrk-test/{pid}-{纳秒}/{id}/{version}/stage-{n}/`（进程独占 + 调用独占，
//! 见 `common/mod.rs` 头注的 F11b/F11c）。wasm 客户机用 `wat` 即时编译，不依赖 wasm32
//! 工具链。

use std::path::PathBuf;
use std::sync::Arc;

use plugin_host::{PluginRegistry, RegistryToolSource};
use semver::Version;
use serde_json::json;
mod common;
use common::{call, load_activate};

/// ping 客户机：log 导入 + 静态输出格（`{"pong":true}`）。
const PING_GUEST: &str = r#"
(module
  (import "locowrk" "log" (func $log (param i32 i32 i32)))
  (memory (export "memory") 2)
  (data (i32.const 1024) "{\"id\":\"wasm-ping\",\"version\":\"0.1.0\",\"abi_version\":1,\"kind\":\"tool\",\"tools\":[{\"name\":\"ping\",\"description\":\"ping\",\"schema\":{\"type\":\"object\"},\"side_effect\":\"Pure\"}]}\00")
  (data (i32.const 2048) "{\"pong\":true}")
  (func $alloc (export "alloc") (param i32) (result i32) (i32.const 4096))
  (func $free (export "free") (param i32 i32))
  (func $describe (export "describe") (result i32) (i32.const 1024))
  (func $init (export "init") (result i32) (i32.const 0))
  (func $call (export "call") (param i32 i32 i32 i32 i32 i32 i32) (result i32)
    (call $log (i32.const 3) (i32.const 2048) (i32.const 12))
    (i32.store (i32.const 4096) (i32.const 2048))
    (i32.store (i32.const 4100) (i32.const 13))
    (i32.const 0))
  (func $shutdown (export "shutdown") (param i32))
)
"#;

/// 把 ping 客户机落成版本目录（wasm_ping.wasm + manifest.json 同目录，
/// packaging.md §3.1 布局），返回 wasm 工件路径。持 build 锁：并发
/// 测试写同一 tmp 文件在 Windows 上会报"文件正在使用"。
fn stage_ping_guest() -> PathBuf {
    let _guard = common::build_lock().lock().unwrap();
    let bytes = wat::parse_str(PING_GUEST).expect("ping wat parses");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tmp = common::run_root().join(format!("tmp-ping-{nanos}.wasm"));
    std::fs::write(&tmp, bytes).unwrap();
    let staged = common::stage(&tmp, "wasm-ping", "0.1.0", "wasm_ping.wasm");
    std::fs::remove_file(&tmp).unwrap();
    staged
}

#[test]
fn wasm_plugin_staged_and_served_via_registry() {
    let wasm = stage_ping_guest();

    let registry = Arc::new(PluginRegistry::new());
    let ver: Version = "0.1.0".parse().unwrap();
    registry.load("wasm-ping", &ver, &wasm).expect("load wasm");
    registry.activate("wasm-ping", &ver).expect("activate");

    // 快照目录（stage-{n}）即 wasm 所在目录：manifest.json/图标的定位基础不受影响，
    // plugin_dir() 只取工件路径的父目录，多一层对 registry 是透明的
    assert_eq!(
        registry.plugin_dir("wasm-ping", &ver),
        Some(wasm.parent().unwrap().to_path_buf())
    );

    let source = RegistryToolSource::new(Arc::clone(&registry));
    let ping = source.resolve("ping").expect("ping in catalog");
    let out = call(ping.as_ref(), json!({})).expect("ping ok");
    assert_eq!(out, json!({ "pong": true }));
}

#[test]
fn native_and_wasm_coexist_in_one_registry() {
    let wasm = stage_ping_guest();

    let registry = Arc::new(PluginRegistry::new());
    load_activate("tool-echo", "1.0.0", &registry);
    let ver: Version = "0.1.0".parse().unwrap();
    registry.load("wasm-ping", &ver, &wasm).expect("load wasm");
    registry.activate("wasm-ping", &ver).expect("activate");

    // 工具目录：两种加载形态的工具共存
    let source = RegistryToolSource::new(Arc::clone(&registry));
    let names: Vec<_> = source.list_tools().iter().map(|s| s.name.clone()).collect();
    assert!(names.contains(&"echo".to_string()), "{names:?}");
    assert!(names.contains(&"ping".to_string()), "{names:?}");

    let echo = source.resolve("echo").expect("native echo");
    let out = call(echo.as_ref(), json!({ "text": "hi" })).expect("echo ok");
    assert_eq!(out["echo"], json!("hi"));

    let ping = source.resolve("ping").expect("wasm ping");
    let out = call(ping.as_ref(), json!({})).expect("ping ok");
    assert_eq!(out, json!({ "pong": true }));
}

/// **F11c 守卫**：同一 `(id, version)` 的两次 `stage()` 必须落到**不同**路径，各自内容
/// 完整。上面两个用例正是病发现地——它们都往 `wasm-ping/0.1.0/` 里复制同一个文件，
/// 一方 `fs::copy` 的 truncate 撞上另一方 `registry.load()` 的读，报文
/// `unexpected end-of-file (at offset 0x0)`。
///
/// 这条断言是**确定性的**（不靠并发运气）：把 `common::stage()` 里的调用级序号摘掉，
/// 两次返回的路径必然相等 → 本用例必红。为什么放本文件而不是 `common/mod.rs`：后者
/// 的 `#[test]` 会被五个测试二进制各编一遍各跑一遍，而竞争的产地就在这里。
#[test]
fn staging_two_calls_never_share_a_target_path() {
    let payload: Vec<u8> = (0..4096).map(|i| (i % 251) as u8).collect();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let src = common::run_root().join(format!("tmp-uniq-{nanos}.wasm"));
    std::fs::write(&src, &payload).unwrap();

    let a = common::stage(&src, "wasm-uniq", "0.1.0", "wasm_uniq.wasm");
    let b = common::stage(&src, "wasm-uniq", "0.1.0", "wasm_uniq.wasm");

    assert_ne!(
        a, b,
        "F11c 复发：两次 stage() 落到同一路径 = 同二进制内的并行用例会在同一文件上互相 truncate"
    );
    assert_eq!(std::fs::read(&a).unwrap(), payload, "第一份快照内容不完整");
    assert_eq!(std::fs::read(&b).unwrap(), payload, "第二份快照内容不完整");

    std::fs::remove_file(&src).unwrap();
}
