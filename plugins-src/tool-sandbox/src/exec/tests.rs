//! 执行层单测：参数钳制 / 真实受限 spawn 全链（git --version，经显式
//! extra_allow 放行——真跑一遍受限令牌 + Job 挂载或其如实降级路径）/
//! 截断提示文案 / 环境缺省解析。杀树路径依赖平台信号语义，不在单测里
//! 模拟（策略层已覆盖拦截面）。

use super::{RunDefaults, arg_u64, env_u64, sandbox_exec};
use plugin_api::serde_json::json;

const DEFAULTS: RunDefaults = RunDefaults {
    timeout_secs: 20,
    max_bytes: 64 * 1024,
    mem_mb: 512,
    max_procs: 64,
    cpu_rate: None,
};

#[test]
fn arg_u64_clamps_and_rejects_non_numeric() {
    assert_eq!(arg_u64(&json!({}), "mem_mb", 512, 4096).unwrap(), 512);
    assert_eq!(
        arg_u64(&json!({ "mem_mb": 99999 }), "mem_mb", 512, 4096).unwrap(),
        4096
    );
    assert!(arg_u64(&json!({ "mem_mb": "x" }), "mem_mb", 512, 4096).is_err());
}

/// 环境缺省解析：未设 = 内置缺省；越上界钳制；低于下界 / 非数字
/// = Err 告警。独立变量名，避免与同二进制里其它 env 测试并行互踩。
#[test]
fn env_u64_unset_clamps_and_rejects() {
    const VAR: &str = "LOCOWRK_SANDBOX_TEST_ENV_U64";
    unsafe { std::env::remove_var(VAR) };
    assert_eq!(env_u64(VAR, 512, 64, 4096).unwrap(), 512, "未设 = 内置缺省");

    unsafe { std::env::set_var(VAR, "256") };
    assert_eq!(env_u64(VAR, 512, 64, 4096).unwrap(), 256, "合法覆盖生效");

    unsafe { std::env::set_var(VAR, "99999") };
    assert_eq!(env_u64(VAR, 512, 64, 4096).unwrap(), 4096, "越上界钳制");

    unsafe { std::env::set_var(VAR, "32") };
    let err = env_u64(VAR, 512, 64, 4096).unwrap_err();
    assert!(err.contains(VAR) && err.contains("below minimum"), "{err}");

    unsafe { std::env::set_var(VAR, "soon") };
    let err = env_u64(VAR, 512, 64, 4096).unwrap_err();
    assert!(err.contains("not a number"), "{err}");

    unsafe { std::env::remove_var(VAR) };
}

#[test]
fn runs_whitelisted_program_end_to_end() {
    // git --version：内置白名单只有 `git status` 子命令条目，这里显式
    // `git` 放行（denylist 外），走通 切词 → 策略 → 受限 spawn →
    // 双管道收集 → json 投影全链
    let out = sandbox_exec(
        &json!({ "command": "git --version" }),
        &["git".to_string()],
        &DEFAULTS,
    )
    .unwrap();
    assert_eq!(out["program"], "git");
    assert_eq!(out["success"], true);
    assert_eq!(out["truncated"], false);
    assert!(out["stdout"].as_str().unwrap().contains("git version"));
    // isolation 如实报告生效档：Windows = restricted-token+job 或
    // 降级 job-only；Unix = process-group+rlimits。任一形态都不为空
    let iso = out["isolation"].as_str().unwrap();
    assert!(iso.contains("job") || iso.contains("rlimits"), "{iso}");
    // 全量限额参数（mem/procs/cpu_rate）同链生效：Job/rlimit 设置失败
    // 会直接 spawn 报错，跑通即证明限额面全部建立成功
    let out = sandbox_exec(
        &json!({ "command": "git --version", "mem_mb": 256, "max_procs": 8, "cpu_rate": 50 }),
        &["git".to_string()],
        &DEFAULTS,
    )
    .unwrap();
    assert_eq!(out["success"], true);
}

#[test]
fn truncation_appends_retry_hint() {
    let out = sandbox_exec(
        &json!({ "command": "git --version", "max_bytes": 10 }),
        &["git".to_string()],
        &DEFAULTS,
    )
    .unwrap();
    assert_eq!(out["truncated"], true);
    let stdout = out["stdout"].as_str().unwrap();
    assert!(stdout.contains("output truncated at 10 bytes"), "{stdout}");
    assert!(stdout.contains("retry"), "{stdout}");
}

#[test]
fn policy_denies_surface_as_tool_errors() {
    // 高危（沙箱内无解锁，引导去 shell）
    let err = sandbox_exec(&json!({ "command": "rm -rf /" }), &[], &DEFAULTS).unwrap_err();
    assert!(err.contains("no unlock switch"), "{err}");
    // 硬锁（即便显式放行也拒）
    let err = sandbox_exec(
        &json!({ "command": "python -V" }),
        &["python".to_string()],
        &DEFAULTS,
    )
    .unwrap_err();
    assert!(err.contains("permanently locked"), "{err}");
    // cwd 不存在
    let err = sandbox_exec(
        &json!({ "command": "git --version", "cwd": "Z:/definitely/missing" }),
        &["git".to_string()],
        &DEFAULTS,
    )
    .unwrap_err();
    assert!(err.contains("is not a directory"), "{err}");
}
