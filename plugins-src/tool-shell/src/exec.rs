//! 进程执行：**无 shell** —— `Command::new(首词).args(其余)` 直接 spawn，
//! 策略层已保证首词在白名单/只读表内。带轮询超时（超时杀**整棵进程树**：
//! Windows Job Object / Unix 进程组——只杀首进程会让孙进程成孤儿，
//! 清单 §2.5）、stdout/stderr 双管道读取线程（防管道满死锁）、输出
//! 截断并附提示文案（清单 §2.6）。

use std::io::Read;
use std::path::{Path, PathBuf};
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

/// cwd 参数解析（存在性校验在这里，根内校验只归只读面）。
fn cwd_of(args: &Value) -> Result<Option<PathBuf>, String> {
    match args.get("cwd").and_then(Value::as_str) {
        None | Some("") => Ok(None),
        Some(dir) => {
            let p = Path::new(dir);
            if !p.is_dir() {
                return Err(format!("shell `cwd` `{dir}` is not a directory"));
            }
            Ok(Some(p.to_path_buf()))
        }
    }
}

/// 审批工具（shell）入口：策略管线（硬锁/高危/白名单）→ 执行。
pub(crate) fn shell_exec(
    args: &Value,
    extra_allow: &[String],
    unlock_risky: bool,
) -> Result<Value, String> {
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `command`".to_string())?;
    let (program, cargs) = match crate::policy::decide(command, extra_allow, unlock_risky) {
        Decision::Allow { program, args } => (program, args),
        Decision::Deny { reason } => return Err(reason),
    };
    let cwd = cwd_of(args)?;
    run(command, program, cargs, args, cwd)
}

/// 只读工具（shell_read）入口：只读表 → 路径校验（fsutil 同源根）→
/// 执行。路径出根在这里被拒（降级：改用 shell 工具走审批）。
pub(crate) fn shell_read_exec(args: &Value) -> Result<Value, String> {
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `command`".to_string())?;
    let (program, cargs) = match crate::policy::decide_readonly(command) {
        Decision::Allow { program, args } => (program, args),
        Decision::Deny { reason } => return Err(reason),
    };
    let cwd = cwd_of(args)?;
    crate::paths::ensure_readonly_scope(&cargs, cwd.as_deref())?;
    run(command, program, cargs, args, cwd)
}

fn run(
    command: &str,
    program: String,
    cargs: Vec<String>,
    args: &Value,
    cwd: Option<PathBuf>,
) -> Result<Value, String> {
    let max_out = arg_u64(args, "max_bytes", OUT_DEFAULT, OUT_MAX)?;
    let timeout = arg_u64(args, "timeout_secs", TIMEOUT_DEFAULT, TIMEOUT_MAX)?;

    let mut cmd = Command::new(&program);
    cmd.args(cargs.iter())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = &cwd {
        cmd.current_dir(dir);
    }
    // Unix：spawn 起新进程组（pgid = 首进程 pid），killpg 才打得着孙进程
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("shell spawn `{program}`: {e}"))?;
    // Windows：进程一出生就挂进 Job（KILL_ON_JOB_CLOSE）——收掉 Job 即
    // 收掉整棵树；正常退出时 Drop 清残留（白名单程序无常驻子进程）。
    #[cfg(windows)]
    let job = crate::exec::job::JobGuard::assign(&child);
    // Unix 才需要记 pid（killpg 用）；Windows 治理走 Job 句柄
    #[cfg(unix)]
    let child_pid = child.id();
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
                    #[cfg(windows)]
                    job.kill_tree();
                    #[cfg(unix)]
                    kill_group(child_pid);
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => return Err(format!("shell `{command}`: wait: {e}")),
        }
    };

    // 收尾先清场再收管道：残留孙进程握着管道会让 rx 永远等不到 EOF
    // （Windows：Drop 触发 KILL_ON_JOB_CLOSE；Unix：对进程组补一发
    // SIGKILL，组长已退则组内孤儿一并收掉，组已空则 ESRCH 静默忽略）。
    #[cfg(windows)]
    drop(job);
    #[cfg(unix)]
    kill_group(child_pid);

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
        "exit_code": status.as_ref().map(|s| json!(s.code())),
        "success": status.as_ref().map(|s| s.success()).unwrap_or(false),
        "timed_out": timed_out,
        "truncated": out_trunc || err_trunc,
        "stdout": stdout,
        "stderr": stderr,
    }))
}

/// Unix：对子进程组补一发 SIGKILL（spawn 时已 process_group(0)）。
#[cfg(unix)]
fn kill_group(pgid: u32) {
    // SAFETY: killpg 只投递信号；组不存在（已全部退出）返回 ESRCH，忽略
    unsafe {
        libc::killpg(pgid as libc::pid_t, libc::SIGKILL);
    }
}

/// Windows 进程树容器（清单 §2.5：TerminateProcess 只杀一个，Job 才能
/// 杀一整棵树）。KILL_ON_JOB_CLOSE——句柄一关（Drop），组内残留进程
/// 全部终止。
#[cfg(windows)]
pub(crate) mod job {
    use std::os::windows::io::AsRawHandle;
    use std::process::Child;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject, TerminateJobObject,
    };

    pub(crate) struct JobGuard(HANDLE);

    impl JobGuard {
        /// 建 Job 并把子进程挂进去；任一步失败降级为旧行为（只杀首进程），
        /// 不让执行器因治理设施缺席而拒绝干活。
        pub(crate) fn assign(child: &Child) -> JobGuard {
            // SAFETY: 全程持合法句柄；失败路径立即 CloseHandle 释放
            unsafe {
                let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
                if job.is_null() {
                    return JobGuard(std::ptr::null_mut());
                }
                let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
                info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
                if SetInformationJobObject(
                    job,
                    JobObjectExtendedLimitInformation,
                    &info as *const _ as *const core::ffi::c_void,
                    std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                ) == 0
                {
                    CloseHandle(job);
                    return JobGuard(std::ptr::null_mut());
                }
                if AssignProcessToJobObject(job, child.as_raw_handle()) == 0 {
                    CloseHandle(job);
                    return JobGuard(std::ptr::null_mut());
                }
                JobGuard(job)
            }
        }

        pub(crate) fn kill_tree(&self) {
            if !self.0.is_null() {
                // SAFETY: 句柄在本对象生命周期内有效
                unsafe {
                    TerminateJobObject(self.0, 1);
                }
            }
        }
    }

    impl Drop for JobGuard {
        fn drop(&mut self) {
            if !self.0.is_null() {
                // SAFETY: 句柄在本对象生命周期内有效
                unsafe {
                    CloseHandle(self.0);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
