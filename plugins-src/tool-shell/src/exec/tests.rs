//! 执行层单测：参数钳制 / 真实 spawn 全链（git --version，经显式
//! extra_allow 放行，不碰进程 env）/ 截断提示文案。杀树路径依赖
//! 平台信号语义，不在单测里模拟（策略与路径层已覆盖拦截面）。

use super::{arg_u64, shell_exec};
use plugin_api::serde_json::json;

#[test]
fn arg_u64_clamps_and_rejects_non_numeric() {
    assert_eq!(arg_u64(&json!({}), "timeout_secs", 20, 120).unwrap(), 20);
    assert_eq!(
        arg_u64(&json!({ "timeout_secs": 999 }), "timeout_secs", 20, 120).unwrap(),
        120
    );
    assert!(arg_u64(&json!({ "timeout_secs": "x" }), "timeout_secs", 20, 120).is_err());
}

#[test]
fn runs_whitelisted_program_end_to_end() {
    // git --version：显式 extra_allow 放行（denylist 外），走通
    // 切词 → 策略 → spawn → 双管道收集 → json 投影
    let out = shell_exec(
        &json!({ "command": "git --version" }),
        &["git --version".to_string()],
        false,
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
    )
    .unwrap();
    assert_eq!(out["truncated"], true);
    let stdout = out["stdout"].as_str().unwrap();
    assert!(stdout.contains("output truncated at 10 bytes"), "{stdout}");
    assert!(stdout.contains("retry"), "{stdout}");
}
