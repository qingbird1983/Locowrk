//! 进程执行：**无 shell** —— `Command::new(首词).args(其余)` 直接 spawn，
//! 策略层/证明层已保证是单条简单命令（shell：结构合法即进审批；shell_read：
//! 可证明只读）。带轮询超时（超时杀**整棵进程树**：
//! Windows Job Object / Unix 进程组——只杀首进程会让孙进程成孤儿，
//! 清单 §2.5）、stdout/stderr 双管道读取线程（防管道满死锁）、输出
//! 截断并附提示文案（清单 §2.6）。

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use plugin_api::HostVtable;
use plugin_api::args::arg_u64;
use plugin_api::serde_json::{Value, json};

use crate::policy::Decision;

/// 输出缺省/上限（字节，stdout 与 stderr 各自计）。
const OUT_DEFAULT: u64 = 64 * 1024;
const OUT_MAX: u64 = 256 * 1024;
/// 超时缺省/上限（秒）。
const TIMEOUT_DEFAULT: u64 = 20;
const TIMEOUT_MAX: u64 = 120;

/// 运行缺省（S2，清单 §3"其他约束给合理默认值随时可调"）：模型 arg 缺省
/// 时取这里的值；来源 = 内置缺省或环境变量覆盖（init 读一次）。
#[derive(Debug, Clone, Copy)]
pub(crate) struct RunDefaults {
    pub(crate) timeout_secs: u64,
    pub(crate) max_bytes: u64,
}

impl RunDefaults {
    /// 环境覆盖：`LOCOWRK_SHELL_TIMEOUT`（秒，1–120）/
    /// `LOCOWRK_SHELL_MAX_BYTES`（字节，1024–256K）。越上界钳到上限；
    /// 解析失败/低于下界回落内置缺省并返回告警（清单 §2.4，不静默）。
    pub(crate) fn from_env() -> (RunDefaults, Vec<String>) {
        let mut warns = Vec::new();
        let timeout_secs = match env_u64("LOCOWRK_SHELL_TIMEOUT", TIMEOUT_DEFAULT, 1, TIMEOUT_MAX) {
            Ok(v) => v,
            Err(w) => {
                warns.push(w);
                TIMEOUT_DEFAULT
            }
        };
        let max_bytes = match env_u64("LOCOWRK_SHELL_MAX_BYTES", OUT_DEFAULT, 1024, OUT_MAX) {
            Ok(v) => v,
            Err(w) => {
                warns.push(w);
                OUT_DEFAULT
            }
        };
        (
            RunDefaults {
                timeout_secs,
                max_bytes,
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
            "tool-shell: {var}={raw} below minimum {min}; keeping {fallback}"
        )),
        Err(_) => Err(format!(
            "tool-shell: {var}={raw} is not a number; keeping {fallback}"
        )),
    }
}

/// cwd 参数解析（存在性校验在这里，根内校验只归只读面）。
///
/// 报文教学（F12b / DESIGN §3.3）：只宣布「不是目录」的拒绝，模型下一步
/// 只能在原地重试或换一个猜出来的路径。这里把病因分两说（是文件 vs 根本
/// 不存在）并给两条真出路：省略 `cwd`（子进程就落在宿主进程的工作目录，
/// `run` 只在 Some 时 `current_dir`）、或先 `fs_list` 上级看清层级。
/// 措辞与 manifest 的 `cwd` 描述同口径（optional / must exist），不写
/// "workspace root"——缺省落点是宿主进程的 cwd，不是工作根。
fn cwd_of(args: &Value) -> Result<Option<PathBuf>, String> {
    match args.get("cwd").and_then(Value::as_str) {
        None | Some("") => Ok(None),
        Some(dir) => {
            let p = Path::new(dir);
            if !p.is_dir() {
                let cause = if p.is_file() {
                    "that path is a file — `cwd` takes the directory to run in, not a file inside it"
                } else {
                    "nothing exists at that path (a directory recalled from an earlier turn or \
                     from memory is a hint to verify, not a guarantee)"
                };
                return Err(format!(
                    "shell `cwd` `{dir}` is not a directory — {cause}. Omit `cwd` to run in the \
                     host process's working directory, or run `fs_list` on the parent directory \
                     to see what is actually there"
                ));
            }
            Ok(Some(p.to_path_buf()))
        }
    }
}

/// 审批工具（shell）入口：策略管线（结构性拒绝，其余放行）→ 执行。
pub(crate) fn shell_exec(args: &Value, defaults: &RunDefaults) -> Result<Value, String> {
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `command`".to_string())?;
    let (program, cargs) = match crate::policy::decide(command) {
        Decision::Allow { program, args } => (program, args),
        Decision::Deny { reason } => return Err(reason),
    };
    let cwd = cwd_of(args)?;
    run(command, program, cargs, args, cwd, defaults)
}

/// 只读工具（shell_read）入口：只读证明（词法/语法/语义，proof.rs）→
/// 路径校验（fsutil 同源根）→ 执行。证明不了（Unsafe/Unknown 一视
/// 同仁）与路径出根都在这里被拒（降级：改用 shell 工具走审批）。
pub(crate) fn shell_read_exec(
    args: &Value,
    defaults: &RunDefaults,
    host: *const HostVtable,
) -> Result<Value, String> {
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `command`".to_string())?;
    let (program, cargs) = match crate::proof::prove(command) {
        crate::proof::Proof::Safe { program, args } => (program, args),
        crate::proof::Proof::Unsafe { reason } | crate::proof::Proof::Unknown { reason } => {
            return Err(reason);
        }
    };
    let cwd = cwd_of(args)?;
    crate::paths::ensure_readonly_scope(&cargs, cwd.as_deref(), host)?;
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
    let timeout = arg_u64(args, "timeout_secs", defaults.timeout_secs, TIMEOUT_MAX)?;

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
