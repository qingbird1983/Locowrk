//! wasm 加载器三态用例：正常（echo / manifest / registry 分发）/
//! 边界（trap、超时预算、缺导出、无 NUL）/ 恶意（wasi 导入拒绝、
//! 内存超限、谎报越界输出）。全部持 env 锁串行：时间预算 env 是
//! 按实例读取的，其他用例加载期间不能被改到。

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use super::WasmLoader;
use super::guests;
use crate::adapter::RegistryToolSource;
use crate::registry::PluginRegistry;
use plugin_api::ToolError;
use plugin_api::tool::SideEffect;
use serde_json::json;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("locowrk-wasm-{tag}-{}-{nanos}", std::process::id()))
}

/// WAT → wasm 字节码文件。
fn write_guest(dir: &Path, name: &str, wat: &str) -> PathBuf {
    let bytes = wat::parse_str(wat).expect("test wat parses");
    let path = dir.join(format!("{name}.wasm"));
    std::fs::write(&path, bytes).expect("write wasm");
    path
}

fn err_msg(e: ToolError) -> String {
    match e {
        ToolError::Failed { message, .. } => message,
        other => other.to_string(),
    }
}

#[test]
fn echo_end_to_end_manifest_and_specs() {
    let _env = ENV_LOCK.lock().unwrap();
    let dir = temp_dir("echo");
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_guest(&dir, "wasm-echo", guests::MAIN_GUEST);

    let inst = WasmLoader::load(&path).expect("load main guest");
    assert_eq!(inst.manifest().id, "wasm-echo");
    assert_eq!(inst.version().to_string(), "0.1.0");
    // side_effect 双形态解析：echo 显式 Pure，其余按缺省 Irreversible
    assert!(matches!(
        inst.spec_of("echo").unwrap().side_effect,
        SideEffect::Pure
    ));
    assert!(matches!(
        inst.spec_of("fail").unwrap().side_effect,
        SideEffect::Irreversible
    ));

    let out = inst
        .call_blocking("echo", &json!({ "x": 1 }))
        .expect("echo");
    assert_eq!(out, json!({ "echo": { "x": 1 } }));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn error_envelope_unknown_tool_and_trap() {
    let _env = ENV_LOCK.lock().unwrap();
    let dir = temp_dir("err");
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_guest(&dir, "wasm-echo", guests::MAIN_GUEST);
    let inst = WasmLoader::load(&path).expect("load");

    // 错误信封 {"error":"boom"} + code ERR_CALL_FAILED
    let msg = err_msg(inst.call_blocking("fail", &json!({})).unwrap_err());
    assert!(msg.contains("boom"), "{msg}");
    // 未知工具：guest 路由到 ERR_UNKNOWN_TOOL
    let msg = err_msg(inst.call_blocking("zzz", &json!({})).unwrap_err());
    assert!(msg.contains("unknown tool"), "{msg}");
    // guest 不可达指令：trap 透传
    let msg = err_msg(inst.call_blocking("trap", &json!({})).unwrap_err());
    assert!(msg.contains("unreachable"), "{msg}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn guest_reporting_output_outside_memory_is_rejected() {
    let _env = ENV_LOCK.lock().unwrap();
    let dir = temp_dir("oob");
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_guest(&dir, "wasm-echo", guests::MAIN_GUEST);
    let inst = WasmLoader::load(&path).expect("load");

    // oob 工具谎报 1 GiB 输出：宿主先校验界内再拷贝
    let msg = err_msg(inst.call_blocking("oob", &json!({})).unwrap_err());
    assert!(msg.contains("outside linear memory"), "{msg}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn epoch_budget_kills_spin_and_resets_next_call() {
    let _env = ENV_LOCK.lock().unwrap();
    // 预算按实例读取：必须在 load 之前设置
    unsafe { std::env::set_var("LOCOWRK_WASM_TIMEOUT_MS", "100") };
    let dir = temp_dir("timeout");
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_guest(&dir, "wasm-echo", guests::MAIN_GUEST);
    let inst = WasmLoader::load(&path).expect("load");

    let started = std::time::Instant::now();
    let msg = err_msg(inst.call_blocking("loop", &json!({})).unwrap_err());
    assert!(msg.contains("time budget"), "{msg}");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "spin killed promptly"
    );
    // 预算是 per-call 的：上次耗尽不污染下一次
    let out = inst
        .call_blocking("echo", &json!({}))
        .expect("echo after timeout");
    assert_eq!(out, json!({ "echo": {} }));

    unsafe { std::env::remove_var("LOCOWRK_WASM_TIMEOUT_MS") };
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_export_names_abi() {
    let _env = ENV_LOCK.lock().unwrap();
    let dir = temp_dir("minimal");
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_guest(&dir, "wasm-min", guests::MINIMAL_GUEST);

    let msg = WasmLoader::load(&path).unwrap_err().to_string();
    assert!(msg.contains("missing export `alloc`"), "{msg}");
    assert!(msg.contains("ABI requires"), "{msg}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn describe_without_nul_is_rejected() {
    let _env = ENV_LOCK.lock().unwrap();
    let dir = temp_dir("nonul");
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_guest(&dir, "wasm-nonul", guests::NO_NUL_GUEST);

    let msg = WasmLoader::load(&path).unwrap_err().to_string();
    assert!(msg.contains("no NUL terminator"), "{msg}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn foreign_import_is_rejected_at_instantiation() {
    let _env = ENV_LOCK.lock().unwrap();
    let dir = temp_dir("wasi");
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_guest(&dir, "wasm-wasi", guests::WASI_GUEST);

    // Linker 只提供 locowrk.log：wasi_* 一律实例化失败
    let msg = WasmLoader::load(&path).unwrap_err().to_string();
    assert!(msg.contains("wasi_snapshot_preview1"), "{msg}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn oversized_memory_is_rejected_by_limiter() {
    let _env = ENV_LOCK.lock().unwrap();
    let dir = temp_dir("bigmem");
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_guest(&dir, "wasm-bigmem", guests::BIGMEM_GUEST);

    let msg = WasmLoader::load(&path).unwrap_err().to_string();
    assert!(msg.contains("memory limits"), "{msg}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn registry_dispatches_wasm_and_serves_tools() {
    let _env = ENV_LOCK.lock().unwrap();
    let dir = temp_dir("registry");
    std::fs::create_dir_all(&dir).unwrap();
    let path = write_guest(&dir, "wasm-echo", guests::MAIN_GUEST);

    // .wasm 扩展名 → WasmLoader（无需显式指定加载器）
    let registry = std::sync::Arc::new(PluginRegistry::new());
    let ver: semver::Version = "0.1.0".parse().unwrap();
    registry
        .load("wasm-echo", &ver, &path)
        .expect("load via registry");
    registry.activate("wasm-echo", &ver).expect("activate");
    assert!(registry.plugin_dir("wasm-echo", &ver).is_some());

    let source = RegistryToolSource::new(std::sync::Arc::clone(&registry));
    let names: Vec<_> = source.list_tools().iter().map(|s| s.name.clone()).collect();
    assert!(names.contains(&"echo".to_string()), "{names:?}");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let ctx = plugin_api::CallCtx::new();
    let tool = source.resolve("echo").unwrap();
    let out = rt
        .block_on(tool.call(&ctx, json!({ "k": "v" })))
        .expect("tool call");
    assert_eq!(out, json!({ "echo": { "k": "v" } }));

    let _ = std::fs::remove_dir_all(&dir);
}
