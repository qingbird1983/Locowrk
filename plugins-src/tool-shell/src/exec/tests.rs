//! 执行层单测：参数钳制 / 真实 spawn 全链（git --version，经显式
//! extra_allow 放行，不碰进程 env）/ 截断提示文案 / 环境默认解析。
//! 杀树路径依赖平台信号语义，不在单测里模拟（策略与路径层已覆盖拦截面）。

use super::{RunDefaults, arg_u64, env_u64, shell_exec};
use plugin_api::serde_json::json;

const DEFAULTS: RunDefaults = RunDefaults {
    timeout_secs: 20,
    max_bytes: 64 * 1024,
};

#[test]
fn arg_u64_clamps_and_rejects_non_numeric() {
    assert_eq!(arg_u64(&json!({}), "timeout_secs", 20, 120).unwrap(), 20);
    assert_eq!(
        arg_u64(&json!({ "timeout_secs": 999 }), "timeout_secs", 20, 120).unwrap(),
        120
    );
    assert!(arg_u64(&json!({ "timeout_secs": "x" }), "timeout_secs", 20, 120).is_err());
}

/// 环境默认解析（S2）：未设 = 内置缺省；越上界钳制；低于下界 / 非数字
/// = Err 告警（调用方回落内置缺省并打日志）。独立变量名，避免与同二进制
/// 里其它 env 测试并行互踩。
#[test]
fn env_u64_unset_clamps_and_rejects() {
    const VAR: &str = "LOCOWRK_SHELL_TEST_ENV_U64";
    // 变量名独立，无需与其它 env 测试互斥（paths/policy 的 env 测试各用各名）
    unsafe { std::env::remove_var(VAR) };
    assert_eq!(env_u64(VAR, 20, 1, 120).unwrap(), 20, "未设 = 内置缺省");

    unsafe { std::env::set_var(VAR, "45") };
    assert_eq!(env_u64(VAR, 20, 1, 120).unwrap(), 45, "合法覆盖生效");

    unsafe { std::env::set_var(VAR, "999") };
    assert_eq!(env_u64(VAR, 20, 1, 120).unwrap(), 120, "越上界钳制");

    unsafe { std::env::set_var(VAR, "0") };
    let err = env_u64(VAR, 20, 1, 120).unwrap_err();
    assert!(err.contains(VAR) && err.contains("below minimum"), "{err}");

    unsafe { std::env::set_var(VAR, "soon") };
    let err = env_u64(VAR, 20, 1, 120).unwrap_err();
    assert!(err.contains("not a number"), "{err}");

    unsafe { std::env::remove_var(VAR) };
}

#[test]
fn runs_whitelisted_program_end_to_end() {
    // git --version：显式 extra_allow 放行（denylist 外），走通
    // 切词 → 策略 → spawn → 双管道收集 → json 投影
    let out = shell_exec(
        &json!({ "command": "git --version" }),
        &["git --version".to_string()],
        false,
        &DEFAULTS,
    )
    .unwrap();
    assert_eq!(out["program"], "git");
    assert_eq!(out["success"], true);
    assert_eq!(out["truncated"], false);
    assert!(out["stdout"].as_str().unwrap().contains("git version"));
}

#[test]
fn truncation_appends_retry_hint() {
    let out = shell_exec(
        &json!({ "command": "git --version", "max_bytes": 10 }),
        &["git --version".to_string()],
        false,
        &DEFAULTS,
    )
    .unwrap();
    assert_eq!(out["truncated"], true);
    let stdout = out["stdout"].as_str().unwrap();
    assert!(stdout.contains("output truncated at 10 bytes"), "{stdout}");
    assert!(stdout.contains("retry"), "{stdout}");
}
