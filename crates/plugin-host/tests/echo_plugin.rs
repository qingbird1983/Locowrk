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

/// 逻辑卸载之后再装载**同一份产物**，必须复用保留区里那个实例，不许第二次 dlopen。
///
/// 为什么单独立一条：`load` 的幂等闸门原本只看在册表，而 `deactivate` 是把 Arc 从
/// 在册表**挪进**保留区（映像仍在内存里）——于是停用一次就把幂等保护弄丢了。Linux
/// 上这条缺口是会崩的：同一份 `.so` 被 dlopen 两次，释放时 dlclose 按 so 名合并引用
/// 计数，先归零的那份把代码页带走，后到的 Drop 跳进空 vtable ⇒ SIGSEGV（复现读数
/// 与根因链登记在 specs/03 的 T18，2026-10-17 本机 WSL 实测）。Windows 上"正在使用
/// 的 dll 卸不掉"碰巧挡住了这一层，所以这台机器上从来没有证据。
///
/// 判读用的是 `Arc::ptr_eq`——它是"有没有第二次 dlopen"的可观测差异：改前这里拿到
/// 的是新实例（两个不同 Arc），改后是同一个。
#[test]
fn a_deactivated_version_revives_from_the_graveyard_instead_of_dlopening_twice() {
    let registry = PluginRegistry::new();
    let v: Version = "0.1.0".parse().unwrap();
    let p = build_and_stage("tool-echo", "0.1.0");

    let first = registry.load("tool-echo", &v, &p).expect("load 0.1.0");
    registry.activate("tool-echo", &v).expect("activate 0.1.0");
    registry.deactivate("tool-echo");
    assert!(
        !registry.has_version("tool-echo", &v),
        "停用后在册表应已摘除（否则这条测试根本没走进保留区）"
    );
    assert!(
        registry.current("tool-echo").is_none(),
        "active 指针应已摘除"
    );

    let again = registry
        .load("tool-echo", &v, &p)
        .expect("停用后重新装载同一份产物");
    assert!(
        std::sync::Arc::ptr_eq(&first, &again),
        "同一 (id, version, path) 的第二次 load 必须复用保留区里的实例，不许再 dlopen"
    );

    // 复用之后调用面照常可用：回退目标没有被"卸走"
    registry
        .activate("tool-echo", &v)
        .expect("re-activate 0.1.0");
    let source = RegistryToolSource::new(std::sync::Arc::new(registry));
    let echo = source.resolve("echo").expect("resolve echo 仍然可用");
    let out = call(echo.as_ref(), json!({ "text": "ping" })).unwrap();
    assert_eq!(out["echo"], "ping");
}
