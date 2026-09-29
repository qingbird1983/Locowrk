//! 端到端集成测试：阶段八⑧ WASM 加载器（§5.8，真实 wasmtime 实例化）。
//!
//! - wasm-ping：WAT 客户机落成 `.wasm` 工件 → registry 按 `.wasm` 分发
//!   → 工具目录 → 调用（含 log 导入与静态输出）
//! - native 与 wasm 混载同一 registry：工具目录共存、各自可解析调用
//!
//! native 侧沿用 tools_batch2 模式：测试自建产物暂存到
//! target/locowrk-test/。wasm 客户机用 `wat` 即时编译，不依赖 wasm32
//! 工具链。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use plugin_api::{Tool, ToolError};
use plugin_host::{PluginRegistry, RegistryToolSource};
use semver::Version;
use serde_json::{Value, json};

fn build_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn workspace_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

fn artifact_name(plugin_id: &str) -> String {
    let stem = plugin_id.replace('-', "_");
    if cfg!(windows) {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}

fn build_and_stage(plugin_id: &str, version: &str) -> PathBuf {
    let _guard = build_lock().lock().unwrap();
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "-p", plugin_id, "--quiet"])
        .env("LOCOWRK_PLUGIN_VERSION", version)
        .status()
        .expect("spawn cargo");
    assert!(status.success(), "cargo build -p {plugin_id} failed");

    let src = workspace_dir()
        .join("target/debug")
        .join(artifact_name(plugin_id));
    assert!(src.exists(), "artifact missing at {}", src.display());
    stage(&src, plugin_id, version, &artifact_name(plugin_id))
}

/// 把工件落成版本目录布局（wasm 客户机不走 cargo build，直接给字节）。
fn stage(src: &Path, plugin_id: &str, version: &str, file_name: &str) -> PathBuf {
    let dst_dir = workspace_dir()
        .join("target/locowrk-test")
        .join(plugin_id)
        .join(version);
    std::fs::create_dir_all(&dst_dir).unwrap();
    let dst = dst_dir.join(file_name);
    std::fs::copy(src, &dst).unwrap();
    dst
}

fn load_activate(plugin_id: &str, version: &str, registry: &Arc<PluginRegistry>) {
    let ver: Version = version.parse().unwrap();
    let path = build_and_stage(plugin_id, version);
    registry.load(plugin_id, &ver, &path).expect("load");
    registry.activate(plugin_id, &ver).expect("activate");
}

fn call(tool: &dyn Tool, args: Value) -> Result<Value, ToolError> {
    let ctx = plugin_api::CallCtx::new();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(tool.call(&ctx, args))
}

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
    let _guard = build_lock().lock().unwrap();
    let bytes = wat::parse_str(PING_GUEST).expect("ping wat parses");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let tmp = workspace_dir()
        .join("target/locowrk-test")
        .join(format!("tmp-ping-{nanos}.wasm"));
    std::fs::write(&tmp, bytes).unwrap();
    let staged = stage(&tmp, "wasm-ping", "0.1.0", "wasm_ping.wasm");
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

    // 版本目录即 wasm 所在目录（manifest.json/图标定位基础不受影响）
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
