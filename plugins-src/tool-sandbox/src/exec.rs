//! 执行管线：策略（policy.rs）放行后受限 spawn（spawn.rs，Windows =
//! 受限令牌 + Job 限额）→ stdout/stderr 双管道读取线程（防管道满
//! 死锁）→ 轮询超时（超时整树终止）→ 收尾清扫 → 输出截断附提示 →
//! JSON 投影（含 `isolation` 如实报告生效隔离档，降级不静默）。
//! 超时/输出/限额的缺省来自内置值，可被环境变量覆盖（init 读一次，
//! 非法回落缺省并告警）、再被模型 arg 覆盖（钳上限）。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use plugin_api::serde_json::{Value, json};

use crate::policy::Decision;
use crate::spawn::{RunLimits, spawn};

/// 输出缺省/上限（字节，stdout 与 stderr 各自计）。
const OUT_DEFAULT: u64 = 64 * 1024;
const OUT_MAX: u64 = 256 * 1024;
/// 超时缺省/上限（秒）。
const TIMEOUT_DEFAULT: u64 = 20;
const TIMEOUT_MAX: u64 = 120;
/// 内存缺省/上限（MB；Job 内存限 / Unix RLIMIT_AS 同值）。
const MEM_DEFAULT: u64 = 512;
const MEM_MAX: u64 = 4096;
/// 进程数缺省/上限（Job 内活跃进程 / Unix RLIMIT_NPROC）。
const PROCS_DEFAULT: u64 = 64;
const PROCS_MAX: u64 = 256;
/// CPU 硬顶占比范围（百分比；缺省不启用）。
const CPU_MIN: u64 = 10;
const CPU_MAX: u64 = 99;

/// 运行缺省（清单 §3"其他约束给合理默认值随时可调"）：模型 arg 缺省
/// 时取这里的值；来源 = 内置缺省或环境变量覆盖（init 读一次）。
#[derive(Debug, Clone, Copy)]
pub(crate) struct RunDefaults {
    pub(crate) timeout_secs: u64,
    pub(crate) max_bytes: u64,
    pub(crate) mem_mb: u64,
    pub(crate) max_procs: u64,
    pub(crate) cpu_rate: Option<u64>,
}

impl RunDefaults {
    /// 环境覆盖：`LOCOWRK_SANDBOX_{TIMEOUT,MAX_BYTES,MEM_MB,MAX_PROCS,
    /// CPU_RATE}`。越上界钳到上限；解析失败/低于下界回落内置缺省并返回
    /// 告警（清单 §2.4，不静默）。
    pub(crate) fn from_env() -> (RunDefaults, Vec<String>) {
        let mut warns = Vec::new();
        let timeout_secs = match env_u64("LOCOWRK_SANDBOX_TIMEOUT", TIMEOUT_DEFAULT, 1, TIMEOUT_MAX)
        {
            Ok(v) => v,
            Err(w) => {
                warns.push(w);
                TIMEOUT_DEFAULT
            }
        };
        let max_bytes = match env_u64("LOCOWRK_SANDBOX_MAX_BYTES", OUT_DEFAULT, 1024, OUT_MAX) {
            Ok(v) => v,
            Err(w) => {
                warns.push(w);
                OUT_DEFAULT
            }
        };
        let mem_mb = match env_u64("LOCOWRK_SANDBOX_MEM_MB", MEM_DEFAULT, 64, MEM_MAX) {
            Ok(v) => v,
            Err(w) => {
                warns.push(w);
                MEM_DEFAULT
            }
        };
        let max_procs = match env_u64("LOCOWRK_SANDBOX_MAX_PROCS", PROCS_DEFAULT, 1, PROCS_MAX) {
            Ok(v) => v,
            Err(w) => {
                warns.push(w);
                PROCS_DEFAULT
            }
        };
        let (cpu_rate, cpu_warn) = cpu_rate_from_env();
        if let Some(w) = cpu_warn {
            warns.push(w);
        }
        (
            RunDefaults {
                timeout_secs,
                max_bytes,
                mem_mb,
                max_procs,
                cpu_rate,
            },
            warns,
        )
    }
}

/// 单个环境变量的缺省解析：未设 = 内置缺省；越上界钳制；其余错误返回
/// 告警文案（调用方回落内置缺省并打日志）。
fn env_u64(var: &str, fallback: u64, min: u64, max: u64) -> Result<u64, String> {
    let raw = match std::env::var(var) {
        Err(_) => return Ok(fallback),
        Ok(raw) => raw,
    };
    match raw.trim().parse::<u64>() {
        Ok(n) if n > max => Ok(max),
        Ok(n) if n >= min => Ok(n),
        Ok(_) => Err(format!(
            "tool-sandbox: {var}={raw} below minimum {min}; keeping {fallback}"
        )),
        Err(_) => Err(format!(
            "tool-sandbox: {var}={raw} is not a number; keeping {fallback}"
        )),
    }
}

/// CPU 占比特殊语义：未设 = 不启用硬顶（None）；合法 10–99 启用；
/// 越上界钳到 99；其余错误 = 保持不启用并告警。
fn cpu_rate_from_env() -> (Option<u64>, Option<String>) {
    match std::env::var("LOCOWRK_SANDBOX_CPU_RATE") {
        Err(_) => (None, None),
        Ok(raw) => match raw.trim().parse::<u64>() {
            Ok(n) if (CPU_MIN..=CPU_MAX).contains(&n) => (Some(n), None),
            Ok(n) if n > CPU_MAX => (
                Some(CPU_MAX),
                Some(format!(
                    "tool-sandbox: LOCOWRK_SANDBOX_CPU_RATE={raw} above maximum \
                     {CPU_MAX}; clamped to {CPU_MAX}"
                )),
            ),
            _ => (
                None,
                Some(format!(
                    "tool-sandbox: LOCOWRK_SANDBOX_CPU_RATE={raw} is not a percent \
                     in {CPU_MIN}-{CPU_MAX}; CPU cap stays off"
                )),
            ),
        },
    }
}

pub(crate) fn arg_u64(args: &Value, key: &str, default: u64, max: u64) -> Result<u64, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .map(|n| n.min(max))
            .ok_or_else(|| format!("field `{key}` must be a non-negative number")),
    }
}

/// arg 覆盖缺省，合成受限执行面的限额档（全部钳上限）。
fn limits_from(args: &Value, defaults: &RunDefaults) -> Result<RunLimits, String> {
    let timeout_secs = arg_u64(args, "timeout_secs", defaults.timeout_secs, TIMEOUT_MAX)?;
    let mem_mb = arg_u64(args, "mem_mb", defaults.mem_mb, MEM_MAX)?;
    let max_procs = arg_u64(args, "max_procs", defaults.max_procs, PROCS_MAX)?;
    let cpu_rate = match args.get("cpu_rate") {
        None | Some(Value::Null) => defaults.cpu_rate,
        Some(v) => {
            let n = v
                .as_u64()
                .ok_or_else(|| "field `cpu_rate` must be a non-negative number".to_string())?;
            Some(n.clamp(CPU_MIN, CPU_MAX))
        }
    };
    Ok(RunLimits {
        timeout_secs,
        mem_mb,
        max_procs,
        cpu_rate,
    })
}

/// cwd 参数解析（存在性校验在这里；沙箱不做根内校验——fs 本就不隔离，
/// 如实交给审批面，见 plugins.md §六能力矩阵）。
fn cwd_of(args: &Value) -> Result<Option<PathBuf>, String> {
    match args.get("cwd").and_then(Value::as_str) {
        None | Some("") => Ok(None),
        Some(dir) => {
            let p = Path::new(dir);
            if !p.is_dir() {
                return Err(format!("sandbox `cwd` `{dir}` is not a directory"));
            }
            Ok(Some(p.to_path_buf()))
        }
    }
}

/// sandbox_run 入口：策略管线（元字符/硬锁/高危/白名单）→ 受限执行。
pub(crate) fn sandbox_exec(
    args: &Value,
    extra_allow: &[String],
    defaults: &RunDefaults,
) -> Result<Value, String> {
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `command`".to_string())?;
    let (program, cargs) = match crate::policy::decide(command, extra_allow) {
        Decision::Allow { program, args } => (program, args),
        Decision::Deny { reason } => return Err(reason),
    };
    let cwd = cwd_of(args)?;
    run(command, program, cargs, args, cwd, defaults)
}

fn run(
    command: &str,
    program: String,
    cargs: Vec<String>,
    args: &Value,
    cwd: Option<PathBuf>,
    defaults: &RunDefaults,
) -> Result<Value, String> {
    let max_out = arg_u64(args, "max_bytes", defaults.max_bytes, OUT_MAX)?;
    let limits = limits_from(args, defaults)?;
    let mut spawned = spawn(&program, &cargs, cwd.as_deref(), &limits)
        .map_err(|e| format!("sandbox spawn `{program}`: {e}"))?;
    let isolation = spawned.isolation.clone();
    let deadline = Instant::now() + Duration::from_secs(limits.timeout_secs);

    // 管道满会卡死子进程——两个读取线程各自搬到缓冲，主线程轮询退出。
    // 线程发送顺序不定，带 tag 区分 stdout / stderr。
    let (tx, rx) = mpsc::channel::<(u8, Vec<u8>)>();
    let (stdout_pipe, stderr_pipe) = spawned.take_pipes();
    for (tag, pipe) in [
        (0u8, Box::new(stdout_pipe) as Box<dyn Read + Send>),
        (1u8, Box::new(stderr_pipe) as Box<dyn Read + Send>),
    ] {
        let tx = tx.clone();
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.take(OUT_MAX + 1).read_to_end(&mut buf);
            let _ = tx.send((tag, buf));
        });
    }
    drop(tx);

    let mut timed_out = false;
    let exit = loop {
        match spawned.poll_wait() {
            Ok(Some(code)) => break code,
            Ok(None) => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    spawned.kill_tree();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(format!("sandbox `{command}`: {e}")),
        }
    };
    // 收尾先清场再收管道：残留孙进程握着管道会让 rx 永远等不到 EOF
    // （Job 收尾终止整树；Unix 对进程组补 SIGKILL）
    spawned.reap();

    // 线程到达顺序不定：按 tag 取，而不是按到达序
    let by_tag: Vec<(u8, Vec<u8>)> = rx.into_iter().collect();
    let pick = |tag: u8| {
        by_tag
            .iter()
            .find(|(t, _)| *t == tag)
            .map(|(_, b)| b.clone())
            .unwrap_or_default()
    };
    let stdout_bytes = pick(0);
    let stderr_bytes = pick(1);
    let truncate = |mut v: Vec<u8>| {
        let truncated = v.len() as u64 > max_out;
        v.truncate(max_out as usize);
        let mut text = String::from_utf8_lossy(&v).into_owned();
        if truncated {
            text.push_str(&format!(
                "\n[output truncated at {max_out} bytes — the result is incomplete; \
                 narrow the command scope (head/tail, a subdirectory, more specific \
                 args) or raise max_bytes and retry]"
            ));
        }
        (text, truncated)
    };
    let (stdout, out_trunc) = truncate(stdout_bytes);
    let (stderr, err_trunc) = truncate(stderr_bytes);

    Ok(json!({
        "command": command,
        "program": program,
        "exit_code": exit,
        "success": exit == Some(0),
        "timed_out": timed_out,
        "truncated": out_trunc || err_trunc,
        "stdout": stdout,
        "stderr": stderr,
        "isolation": isolation,
    }))
}

#[cfg(test)]
mod tests;
