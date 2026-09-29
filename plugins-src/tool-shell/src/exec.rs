//! 进程执行：**无 shell** —— `Command::new(首词).args(其余)` 直接 spawn，
//! 策略层已保证首词在白名单内。带轮询超时（超时 kill）、stdout/stderr
//! 双管道读取线程（防管道满死锁）、输出截断。

use std::io::Read;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use plugin_api::serde_json::{Value, json};

use crate::policy::Decision;

/// 输出缺省/上限（字节，stdout 与 stderr 各自计）。
const OUT_DEFAULT: u64 = 64 * 1024;
const OUT_MAX: u64 = 256 * 1024;
/// 超时缺省/上限（秒）。
const TIMEOUT_DEFAULT: u64 = 20;
const TIMEOUT_MAX: u64 = 120;

pub(crate) fn arg_u64(args: &Value, key: &str, default: u64, max: u64) -> Result<u64, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(default),
        Some(v) => v
            .as_u64()
            .map(|n| n.min(max))
            .ok_or_else(|| format!("field `{key}` must be a non-negative number")),
    }
}

pub(crate) fn shell_exec(args: &Value, extra_allow: &[String]) -> Result<Value, String> {
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `command`".to_string())?;
    let (program, mut cargs) = match crate::policy::decide(command, extra_allow) {
        Decision::Allow { program, args } => (program, args),
        Decision::Deny { reason } => return Err(reason),
    };

    let max_out = arg_u64(args, "max_bytes", OUT_DEFAULT, OUT_MAX)?;
    let timeout = arg_u64(args, "timeout_secs", TIMEOUT_DEFAULT, TIMEOUT_MAX)?;
    let cwd = match args.get("cwd").and_then(Value::as_str) {
        None | Some("") => None,
        Some(dir) => {
            let p = std::path::Path::new(dir);
            if !p.is_dir() {
                return Err(format!("shell `cwd` `{dir}` is not a directory"));
            }
            Some(p.to_path_buf())
        }
    };

    let mut cmd = Command::new(&program);
    cmd.args(cargs.drain(..))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = &cwd {
        cmd.current_dir(dir);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("shell spawn `{program}`: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(timeout);

    // 管道满会卡死子进程——两个读取线程各自搬到缓冲，主线程轮询退出。
    // 线程发送顺序不定，带 tag 区分 stdout / stderr。
    let (tx, rx) = mpsc::channel::<(u8, Vec<u8>)>();
    let stdout_pipe = child.stdout.take().expect("stdout piped");
    let stderr_pipe = child.stderr.take().expect("stderr piped");
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
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    timed_out = true;
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(format!("shell `{command}`: wait: {e}")),
        }
    };

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
        (String::from_utf8_lossy(&v).into_owned(), truncated)
    };
    let (stdout, out_trunc) = truncate(stdout_bytes);
    let (stderr, err_trunc) = truncate(stderr_bytes);

    Ok(json!({
        "command": command,
        "program": program,
        "exit_code": status.as_ref().map(|s| json!(s.code())),
        "success": status.as_ref().map(|s| s.success()).unwrap_or(false),
        "timed_out": timed_out,
        "truncated": out_trunc || err_trunc,
        "stdout": stdout,
        "stderr": stderr,
    }))
}
