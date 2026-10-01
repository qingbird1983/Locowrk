//! 沙箱进程 spawn 接口层。平台分工：
//! - Windows（win.rs）：受限令牌（CreateRestrictedToken）+ JobObject
//!   限额（内存 / 进程数 / CPU 占比 / 剪贴板钩子 UI 限制 / 树治理）
//! - Unix（unix.rs）：pre_exec setrlimit（AS / NPROC / CPU）+ 进程组
//!
//! 这层收敛统一接口：调用方拿到 `Spawned`（双管道 + 轮询等待 + 整树
//! 终止）与**如实描述生效档**的 `isolation` 字符串——令牌或限额缺席
//! 必须写进返回值与日志（降级不静默，清单 §2.4）。

use std::fs::File;
use std::path::Path;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod win;

/// 本次执行的限额档（从 arg/env 合成；timeout/max_bytes 留在 exec 层，
/// 只有 unix 的 RLIMIT_CPU 需要timeout——Windows 超时走轮询终止）。
#[derive(Debug, Clone, Copy)]
pub(crate) struct RunLimits {
    pub(crate) timeout_secs: u64,
    pub(crate) mem_mb: u64,
    pub(crate) max_procs: u64,
    pub(crate) cpu_rate: Option<u64>,
}

pub(crate) struct Spawned {
    #[cfg(windows)]
    imp: win::Imp,
    #[cfg(unix)]
    imp: unix::Imp,
    stdout: Option<File>,
    stderr: Option<File>,
    /// 生效隔离档（如实报告）：`restricted-token+job` /
    /// `job-only (restricted token unavailable: …)` / `process-group+rlimits`。
    pub(crate) isolation: String,
}

impl Spawned {
    /// 双管道搬去读取线程（各取一次；管道满不搬会卡死子进程）。
    pub(crate) fn take_pipes(&mut self) -> (File, File) {
        (
            self.stdout.take().expect("stdout piped"),
            self.stderr.take().expect("stderr piped"),
        )
    }

    /// 非阻塞轮询：None = 还在跑；Some(Some(code)) = 退出码；
    /// Some(None) = 已退出但拿不到退出码。
    pub(crate) fn poll_wait(&mut self) -> Result<Option<Option<i32>>, String> {
        self.imp.poll_wait()
    }

    /// 超时/收尾的整树终止（幂等；收尾时也调一次清残留孙进程）。
    pub(crate) fn kill_tree(&self) {
        self.imp.kill_tree();
    }

    /// 收尾清扫：补一发整树终止并回收子进程，让读取线程能拿到 EOF
    /// （残留孙进程握着管道会让 rx 永远等不到 EOF——tool-shell 同款坑）。
    pub(crate) fn reap(&mut self) {
        self.imp.reap();
    }
}

pub(crate) fn spawn(
    program: &str,
    args: &[String],
    cwd: Option<&Path>,
    limits: &RunLimits,
) -> Result<Spawned, String> {
    #[cfg(windows)]
    {
        win::spawn_process(program, args, cwd, limits)
    }
    #[cfg(unix)]
    {
        unix::spawn_process(program, args, cwd, limits)
    }
}
