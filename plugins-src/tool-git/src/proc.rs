//! git CLI 进程执行：`Command::new("git")` argv 直 spawn（无 shell——参数
//! 由本插件构造，`--` 分隔符挡选项注入）。剥离可重定向仓库位置的环境
//! 变量（GIT_DIR 等——否则 `-C` 的路径校验可被环境绕过），轮询超时 kill，
//! stdout/stderr 读取线程防管道满死锁（与 tool-shell exec.rs 同款）。

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// 单次 git 调用超时（秒）——push 无凭据时可能挂起，超时兜底。
const TIMEOUT_SECS: u64 = 30;
/// 输出上限（字节，stdout / stderr 各自计）。
const OUT_MAX: u64 = 256 * 1024;

pub(crate) struct GitOut {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub truncated: bool,
}

/// 会重定向仓库位置 / 索引的环境变量：`git -C <repo>` 的允许根校验必须
/// 以真实仓库为准，这些变量一律从子进程环境剥掉（fail-closed）。
const REDIRECT_VARS: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_COMMON_DIR",
    "GIT_NAMESPACE",
];

pub(crate) fn run_git(repo: &Path, argv: &[&str]) -> Result<GitOut, String> {
    let mut cmd = Command::new("git");
    cmd.arg("-C").arg(repo).args(argv);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    for var in REDIRECT_VARS {
        cmd.env_remove(var);
    }
    // 无终端凭据提示：挂起交给超时 kill 之前先快速失败
    cmd.env("GIT_TERMINAL_PROMPT", "0");

    let mut child = cmd.spawn().map_err(|e| format!("git spawn: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(TIMEOUT_SECS);

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
            Err(e) => return Err(format!("git {argv:?}: wait: {e}")),
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
    let decode = |mut v: Vec<u8>| -> (String, bool) {
        let truncated = v.len() as u64 > OUT_MAX;
        v.truncate(OUT_MAX as usize);
        (String::from_utf8_lossy(&v).into_owned(), truncated)
    };
    let (stdout, out_trunc) = decode(pick(0));
    let (stderr, err_trunc) = decode(pick(1));
    Ok(GitOut {
        code: status.as_ref().and_then(|s| s.code()),
        stdout,
        stderr,
        timed_out,
        truncated: out_trunc || err_trunc,
    })
}

/// 非零退出 / 超时 → 统一格式 Err（带 stderr 最后一行，模型可读）。
pub(crate) fn require_ok(out: &GitOut, what: &str) -> Result<(), String> {
    if out.timed_out {
        return Err(format!("git {what}: timed out after {TIMEOUT_SECS}s"));
    }
    if out.code != Some(0) {
        // git 的报错末尾常带空行：取最后一条非空行，模型才看得到原因
        let tail = out
            .stderr
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .to_string();
        return Err(format!("git {what} failed: {tail}"));
    }
    Ok(())
}
