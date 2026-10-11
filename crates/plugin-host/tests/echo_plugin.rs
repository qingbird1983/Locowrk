//! 端到端集成测试：真实 dlopen tool-echo 双版本，覆盖阶段三验收链
//! （DESIGN.md §14：加载 0.1.0 → 激活 0.2.0 → 回退 0.1.0，全程不重启）。
//!
//! 测试自行构建插件产物（改 LOCOWRK_PLUGIN_VERSION 触发重编）并暂存到
//! target/locowrk-test/{pid}-{纳秒}/，且每次 stage() 落一个独占快照目录
//! （F11b/F11c，形状与成因见 `common/mod.rs` 头注），不依赖仓库 plugins/ 目录的
//! 预暂存状态。
//! 构建样板（build_lock / build_and_stage / call / err_msg_ref）来自 `mod common`；
//! 那把构建锁只串行化**本测试二进制内**的用例，跨二进制不互斥（现状见 common 头注）。

use plugin_host::{DylibLoader, LoadError, PluginRegistry, RegistryToolSource};
use semver::Version;
use serde_json::json;
mod common;
use common::{build_and_stage, call, err_msg_ref};

#[test]
fn echo_load_activate_rollback_without_restart() {
    let registry = std::sync::Arc::new(PluginRegistry::new());
    let v_old: Version = "0.1.0".parse().unwrap();
    let v_new: Version = "0.2.0".parse().unwrap();

    // ---- 加载 0.1.0 并激活：plain / upper 可用，reverse 尚不存在 ----
    let p_old = build_and_stage("tool-echo", "0.1.0");
    registry
        .load("tool-echo", &v_old, &p_old)
        .expect("load 0.1.0");
    registry
        .activate("tool-echo", &v_old)
        .expect("activate 0.1.0");
    assert_eq!(registry.current("tool-echo").unwrap().version(), &v_old);

    let source = RegistryToolSource::new(std::sync::Arc::clone(&registry));
    let echo = source.resolve("echo").expect("resolve echo");

    let out = call(echo.as_ref(), json!({"text": "Hello, Locowrk!"})).unwrap();
    assert_eq!(out["echo"], "Hello, Locowrk!");
    assert_eq!(out["plugin_version"], "0.1.0");

    let out = call(echo.as_ref(), json!({"text": "hello", "mode": "upper"})).unwrap();
    assert_eq!(out["echo"], "HELLO");

    let err = call(echo.as_ref(), json!({"text": "hello", "mode": "reverse"})).unwrap_err();
    assert!(err_msg_ref(&err).contains("does not support mode=`reverse`"));

    // ---- 加载 0.2.0 并激活（多版本并存，切换 = 换指针）----
    let p_new = build_and_stage("tool-echo", "0.2.0");
    registry
        .load("tool-echo", &v_new, &p_new)
        .expect("load 0.2.0");
    assert!(registry.has_version("tool-echo", &v_old), "旧版本仍并存");
    registry
        .activate("tool-echo", &v_new)
        .expect("activate 0.2.0");

    let echo13 = source.resolve("echo").expect("resolve echo@0.2.0");
    assert_eq!(echo13.plugin_version(), &v_new);
    let out = call(echo13.as_ref(), json!({"text": "Hello", "mode": "reverse"})).unwrap();
    assert_eq!(out["echo"], "olleH");
    assert_eq!(out["plugin_version"], "0.2.0");

    // panic 模式：插件侧 catch_unwind → ERR_PANIC → 宿主 ToolError
    let err = call(echo13.as_ref(), json!({"text": "x", "mode": "panic"})).unwrap_err();
    assert!(err_msg_ref(&err).contains("panicked"));

    // ---- 回退 0.1.0（不重启）：reverse 立刻变回不可用 ----
    let rolled = registry.rollback("tool-echo").expect("rollback");
    assert_eq!(rolled, v_old);
    // 已是最早版本，再回退即 NoPreviousVersion
    assert!(matches!(
        registry.rollback("tool-echo"),
        Err(LoadError::NoPreviousVersion(_))
    ));
    let echo12 = source.resolve("echo").expect("resolve echo@0.1.0");
    assert_eq!(echo12.plugin_version(), &v_old);
    let err = call(echo12.as_ref(), json!({"text": "Hello", "mode": "reverse"})).unwrap_err();
    assert!(err_msg_ref(&err).contains("does not support mode=`reverse`"));

    let states = registry.list_states();
    assert_eq!(states.len(), 1);
    assert_eq!(states[0].active.as_ref(), Some(&v_old));
    assert_eq!(states[0].loaded, vec![v_old.clone(), v_new.clone()]);
}

#[test]
fn loader_rejects_mismatched_identity_and_missing_files() {
    let p_new = build_and_stage("tool-echo", "0.2.0");
    let v_old: Version = "0.1.0".parse().unwrap();

    // manifest 自述 id 与登记键不一致 → 拒载
    let registry = PluginRegistry::new();
    let err = registry
        .load("tool-other", &v_old, &p_new)
        .expect_err("id mismatch must fail");
    assert!(matches!(err, LoadError::IdMismatch { .. }));

    // manifest 自述版本与登记键不一致 → 拒载（防止目录名与产物脱钩）
    let err = registry
        .load("tool-echo", &v_old, &p_new)
        .expect_err("version mismatch must fail");
    assert!(matches!(err, LoadError::VersionMismatch { .. }));

    // 打不开的库 → Open 错误（"加载即失败"豁免路径，不留句柄）
    let err = DylibLoader::load(std::path::Path::new("definitely/not/here.dll"))
        .expect_err("missing dylib must fail");
    assert!(matches!(err, LoadError::Open { .. }));
}
