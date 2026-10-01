//! Unix 受限 spawn：std Command + `pre_exec` setrlimit（`RLIMIT_AS`
//! 地址空间 / `RLIMIT_NPROC` 进程数 / `RLIMIT_CPU` CPU 秒硬顶）+
//! `process_group(0)`（killpg 整树治理，tool-shell 同款）。
//!
//! 如实分级：Unix 权限模型下子进程本就继承调用者 uid、无特权可削
//! （无令牌概念）；`RLIMIT_NPROC` 是 per-uid 计数（同 uid 全部进程
//! 共享，非本进程树独占）——isolation 字符串统一标注
//! `process-group+rlimits`，能力矩阵见 plugins.md §六。

use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use crate::spawn::{RunLimits, Spawned};

pub(crate) struct Imp {
    child: Child,
}

impl Imp {
    pub(crate) fn poll_wait(&mut self) -> Result<Option<Option<i32>>, String> {
        match self.child.try_wait() {
            Ok(Some(status)) => Ok(Some(status.code())),
            Ok(None) => Ok(None),
            Err(e) => Err(format!("wait: {e}")),
        }
    }

    pub(crate) fn kill_tree(&self) {
        kill_group(self.child.id());
    }

    pub(crate) fn reap(&mut self) {
        kill_group(self.child.id());
        let _ = self.child.try_wait();
    }
}

impl Drop for Imp {
    fn drop(&mut self) {
        // reap 已回收过则立即返回；兜底再收一次防僵尸
        let _ = self.child.try_wait();
    }
}

pub(crate) fn spawn_process(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    limits: &RunLimits,
) -> Result<Spawned, String> {
    let mem_bytes = limits.mem_mb * 1024 * 1024;
    let max_procs = limits.max_procs;
    let cpu_secs = limits.timeout_secs + 5; // 软硬同值；SIGKILL 硬顶
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    // spawn 起新进程组（pgid = 首进程 pid），killpg 才打得着孙进程
    cmd.process_group(0);
    // SAFETY: setrlimit 是 async-signal-safe 系统调用；闭包只捕获 Copy 值
    unsafe {
        cmd.pre_exec(move || {
            let mem = libc::rlimit {
                rlim_cur: mem_bytes,
                rlim_max: mem_bytes,
            };
            let procs = libc::rlimit {
                rlim_cur: max_procs,
                rlim_max: max_procs,
            };
            let cpu = libc::rlimit {
                rlim_cur: cpu_secs,
                rlim_max: cpu_secs,
            };
            if libc::setrlimit(libc::RLIMIT_AS, &mem) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            if libc::setrlimit(libc::RLIMIT_NPROC, &procs) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            libc::setrlimit(libc::RLIMIT_CPU, &cpu);
            Ok(())
        });
    }
    let child = cmd.spawn().map_err(|e| format!("spawn `{program}`: {e}"))?;
    let stdout = child.stdout.take().map(std::fs::File::from);
    let stderr = child.stderr.take().map(std::fs::File::from);
    Ok(Spawned {
        imp: Imp { child },
        stdout,
        stderr,
        isolation: "process-group+rlimits".to_string(),
    })
}

/// 对子进程组补一发 SIGKILL（组不存在=已全部退出，ESRCH 静默忽略）。
fn kill_group(pgid: u32) {
    // SAFETY: killpg 只投递信号，无指针参与
    unsafe {
        libc::killpg(pgid as libc::pid_t, libc::SIGKILL);
    }
}
