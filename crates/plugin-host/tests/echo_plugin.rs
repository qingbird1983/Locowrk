//! 端到端集成测试：真实 dlopen tool-echo 双版本，覆盖阶段三验收链
//! （DESIGN.md §14：加载 1.2.0 → 激活 1.3.0 → 回退 1.2.0，全程不重启）。
//!
//! 测试自行构建插件产物（改 LOCOWRK_PLUGIN_VERSION 触发重编）并暂存到
//! target/locowrk-test/，不依赖仓库 plugins/ 目录的预暂存状态。
//! 所有 cargo 调用串行执行（同一把构建锁），避免 env 变量触发的重编互相踩踏。

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

use plugin_api::{Tool, ToolError};
use plugin_host::{DylibLoader, LoadError, PluginRegistry, RegistryToolSource};
use semver::Version;
use serde_json::json;

/// 串行化构建（LOCOWRK_PLUGIN_VERSION 变更会重编同一产物路径）。
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

fn artifact_name() -> &'static str {
    if cfg!(windows) {
        "tool_echo.dll"
    } else if cfg!(target_os = "macos") {
        "libtool_echo.dylib"
    } else {
        "libtool_echo.so"
    }
}

/// 以指定版本构建 tool-echo，并把产物复制到独立暂存目录。
fn build_and_stage(version: &str) -> PathBuf {
    let _guard = build_lock().lock().unwrap();
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "-p", "tool-echo", "--quiet"])
        .env("LOCOWRK_PLUGIN_VERSION", version)
        .status()
        .expect("spawn cargo");
    assert!(
        status.success(),
        "cargo build -p tool-echo ({version}) failed"
    );

    let src = workspace_dir().join("target/debug").join(artifact_name());
    assert!(src.exists(), "artifact missing at {}", src.display());
    let dst_dir = workspace_dir()
        .join("target/locowrk-test/tool-echo")
        .join(version);
    std::fs::create_dir_all(&dst_dir).unwrap();
    let dst = dst_dir.join(artifact_name());
    std::fs::copy(&src, &dst).unwrap();
    dst
}

fn call(tool: &dyn Tool, args: serde_json::Value) -> Result<serde_json::Value, ToolError> {
    let ctx = plugin_api::CallCtx::new();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(tool.call(&ctx, args))
}

fn echo_error(err: &ToolError) -> String {
    match err {
        ToolError::Failed { message, .. } => message.clone(),
        other => other.to_string(),
    }
}

#[test]
fn echo_load_activate_rollback_without_restart() {
    let registry = std::sync::Arc::new(PluginRegistry::new());
    let v12: Version = "1.2.0".parse().unwrap();
    let v13: Version = "1.3.0".parse().unwrap();

    // ---- 加载 1.2.0 并激活：plain / upper 可用，reverse 尚不存在 ----
    let p12 = build_and_stage("1.2.0");
    registry.load("tool-echo", &v12, &p12).expect("load 1.2.0");
    registry
        .activate("tool-echo", &v12)
        .expect("activate 1.2.0");
    assert_eq!(registry.current("tool-echo").unwrap().version(), &v12);

    let source = RegistryToolSource::new(std::sync::Arc::clone(&registry));
    let echo = source.resolve("echo").expect("resolve echo");

    let out = call(echo.as_ref(), json!({"text": "Hello, Locowrk!"})).unwrap();
    assert_eq!(out["echo"], "Hello, Locowrk!");
    assert_eq!(out["plugin_version"], "1.2.0");

    let out = call(echo.as_ref(), json!({"text": "hello", "mode": "upper"})).unwrap();
    assert_eq!(out["echo"], "HELLO");

    let err = call(echo.as_ref(), json!({"text": "hello", "mode": "reverse"})).unwrap_err();
    assert!(echo_error(&err).contains("does not support mode=`reverse`"));

    // ---- 加载 1.3.0 并激活（多版本并存，切换 = 换指针）----
    let p13 = build_and_stage("1.3.0");
    registry.load("tool-echo", &v13, &p13).expect("load 1.3.0");
    assert!(registry.has_version("tool-echo", &v12), "旧版本仍并存");
    registry
        .activate("tool-echo", &v13)
        .expect("activate 1.3.0");

    let echo13 = source.resolve("echo").expect("resolve echo@1.3.0");
    assert_eq!(echo13.plugin_version(), &v13);
    let out = call(echo13.as_ref(), json!({"text": "Hello", "mode": "reverse"})).unwrap();
    assert_eq!(out["echo"], "olleH");
    assert_eq!(out["plugin_version"], "1.3.0");

    // panic 模式：插件侧 catch_unwind → ERR_PANIC → 宿主 ToolError
    let err = call(echo13.as_ref(), json!({"text": "x", "mode": "panic"})).unwrap_err();
    assert!(echo_error(&err).contains("panicked"));

    // ---- 回退 1.2.0（不重启）：reverse 立刻变回不可用 ----
    let rolled = registry.rollback("tool-echo").expect("rollback");
    assert_eq!(rolled, v12);
    // 已是最早版本，再回退即 NoPreviousVersion
    assert!(matches!(
        registry.rollback("tool-echo"),
        Err(LoadError::NoPreviousVersion(_))
    ));
    let echo12 = source.resolve("echo").expect("resolve echo@1.2.0");
    assert_eq!(echo12.plugin_version(), &v12);
    let err = call(echo12.as_ref(), json!({"text": "Hello", "mode": "reverse"})).unwrap_err();
    assert!(echo_error(&err).contains("does not support mode=`reverse`"));

    let states = registry.list_states();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].active.as_ref(), Some(&v12));
    assert_eq!(states[0].loaded, vec![v12.clone(), v13.clone()]);
}

#[test]
fn loader_rejects_mismatched_identity_and_missing_files() {
    let p13 = build_and_stage("1.3.0");
    let v12: Version = "1.2.0".parse().unwrap();

    // manifest 自述 id 与登记键不一致 → 拒载
    let registry = PluginRegistry::new();
    let err = registry
        .load("tool-other", &v12, &p13)
        .expect_err("id mismatch must fail");
    assert!(matches!(err, LoadError::IdMismatch { .. }));

    // manifest 自述版本与登记键不一致 → 拒载（防止目录名与产物脱钩）
    let err = registry
        .load("tool-echo", &v12, &p13)
        .expect_err("version mismatch must fail");
    assert!(matches!(err, LoadError::VersionMismatch { .. }));

    // 打不开的库 → Open 错误（"加载即失败"豁免路径，不留句柄）
    let err = DylibLoader::load(std::path::Path::new("definitely/not/here.dll"))
        .expect_err("missing dylib must fail");
    assert!(matches!(err, LoadError::Open { .. }));
}
