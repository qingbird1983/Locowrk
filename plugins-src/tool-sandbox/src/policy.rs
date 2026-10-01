//! sandbox 命令策略（与 tool-shell 同源三道闸；差异见下）：
//! - **白名单**：内置 `ls` / `cat` / `grep` / `git status`，
//!   `LOCOWRK_SANDBOX_ALLOW` 显式扩展（`prog` / `prog sub`）
//! - **高危清单**：rm / sudo / chmod / dd 等——**在沙箱内一律拒绝、
//!   不设解锁开关**。沙箱对文件系统不隔离（⑯ 方案定稿能力矩阵），
//!   解锁 rm 在这里毫无增益，宁严勿松；引导走 `shell` 的解锁路径
//! - **硬锁清单**：解释器 / 包装 / 网络 / 代码执行面 LOLBin——沙箱
//!   连网络都不隔离，硬锁必须原样保留，永不放行、不给开关
//!
//! denylist 永远赢：白名单（含显式配置）撞上两级清单一律被压制，
//! 启动期经 conflicting_allow() 列出并 warn（清单 §2.4，不静默）。
//!
//! 无 shell 解析：命令串先过元字符检查（管道 / 命令替换 / 顺序执行
//! 直接拒绝），再引号感知切词，**首词即程序**直接 spawn——不存在
//! `curl | sh` 的注入面，元字符检查是纵深防御。
//!
//! 与 tool-shell 的表内容刻意同源（源码拷贝，不抽公共 crate——插件
//! 只依赖 plugin-api）；两处清单必须同步演进，注释指认 tool-shell
//! policy.rs 为内容真源。

/// 审批工具（sandbox_run）内置白名单（与 tool-shell 的 plugins.md 原文
/// 四条一致）。
pub(crate) const BASE_ALLOW: &[&str] = &["ls", "cat", "grep", "git status"];

/// 硬锁清单（内容真源 = tool-shell policy.rs DENY_HARD；拷贝须同步）：
/// 解释器 / 包装 / 网络 / 代码执行面。即便加进 LOCOWRK_SANDBOX_ALLOW
/// 也直接拒绝，且没有解锁开关——沙箱不隔离网络与文件系统，这几类
/// 的间接执行面在沙箱内不减分毫。
pub(crate) const DENY_HARD: &[&str] = &[
    // 网络（能外联，绕过 tool-fetch 的 SSRF 防护）
    "curl",
    "wget",
    "ftp",
    "scp",
    "ssh", //
    "nc",
    "ncat",
    "netcat",
    "telnet",
    "rsync", //
    // 解释器（能执行任意代码）
    "powershell",
    "pwsh",
    "cmd",
    "cmdlet",
    "sh",
    "bash",
    "zsh",
    "fish",
    "dash",
    "csh",
    "tcsh", //
    "python",
    "python3",
    "pip",
    "pip3",
    "node",
    "npm",
    "npx",
    "deno",
    "bun",
    "ruby",
    "perl",
    "php",
    "lua", //
    // 包装（能间接执行其他命令）
    "xargs",
    "env",
    "find",
    "nice",
    "timeout",
    "nohup",
    "time", //
    // 代码执行面 LOLBin（Windows / macOS）
    "wscript",
    "cscript",
    "mshta",
    "rundll32",
    "osascript", //
    "regsvr32",
    "msiexec",
    "certutil",
    "bitsadmin", //
];

/// 高危清单（内容真源 = tool-shell policy.rs DENY_RISKY；拷贝须同步）：
/// rm / sudo / chmod / dd 等"用户能判断后果"的破坏面。沙箱内**无解锁
/// 开关**（文件系统不隔离，解锁无增益）——一律拒绝并引导走 `shell`。
pub(crate) const DENY_RISKY: &[&str] = &[
    // 删除
    "rm",
    "del",
    "rd",
    "rmdir",
    "srm",
    "shred", //
    // 提权
    "sudo",
    "su",
    "doas",
    "runas", //
    // 磁盘 / 挂载
    "dd",
    "mkfs",
    "fdisk",
    "diskpart",
    "format",
    "mount", //
    // 电源
    "shutdown",
    "reboot",
    "halt", //
    // 进程
    "kill",
    "killall",
    "pkill",
    "taskkill", //
    // 权限属性
    "chmod",
    "chown",
    "chattr",
    "attrib",
    "icacls",
    "cacls",
    "takeown", //
    // 注册表
    "reg",
    "regedit", //
    // 系统面
    "chroot",
    "systemctl", //
];

/// 元字符：管道 / 顺序执行 / 重定向 / 命令替换 / 变量展开。出现在命令串
/// 里就整条拒绝（不进审批）。
const METACHARS: &[char] = &['|', '&', ';', '<', '>', '`', '$', '\n', '\r'];

/// 策略判定（含原因，错误信息要能教模型怎么改）。
pub(crate) enum Decision {
    Allow { program: String, args: Vec<String> },
    Deny { reason: String },
}

/// init 时读一次环境配置（`prog` / `prog sub`，分号或逗号分隔）。
pub(crate) fn extra_allow_from_env() -> Vec<String> {
    std::env::var("LOCOWRK_SANDBOX_ALLOW")
        .unwrap_or_default()
        .split([';', ','])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// 程序名是否落在任一级清单（两级清单语义同为"白名单永远进不来"）。
pub(crate) fn is_denied(program_lc: &str) -> bool {
    DENY_HARD.contains(&program_lc) || DENY_RISKY.contains(&program_lc)
}

/// 启动期冲突检测（清单 §2.4）：显式配置里被两级清单压制的条目。
/// denylist 永远赢不变，这里只负责"不静默"。
pub(crate) fn conflicting_allow(extra: &[String]) -> Vec<String> {
    extra
        .iter()
        .filter(|entry| {
            let prog = entry
                .split_ascii_whitespace()
                .next()
                .unwrap_or("")
                .to_ascii_lowercase();
            is_denied(&prog)
        })
        .cloned()
        .collect()
}

/// sandbox_run 的完整判定管线：元字符 → 切词 → 硬锁 → 高危（无解锁，
/// 一律拒绝）→ 白名单。
pub(crate) fn decide(command: &str, extra_allow: &[String]) -> Decision {
    if command.contains(METACHARS) {
        return Decision::Deny {
            reason: "sandbox command contains shell metacharacters (| & ; < > ` $ or \
                     newline); pass a single simple command — pipes and command \
                     substitution are not allowed"
                .into(),
        };
    }
    let Some((program, rest)) = split_command(command) else {
        return Decision::Deny {
            reason: "sandbox command is empty".into(),
        };
    };
    let program_lc = program.to_ascii_lowercase();
    if DENY_HARD.iter().any(|d| *d == program_lc) {
        return Decision::Deny {
            reason: format!(
                "`{program_lc}` is permanently locked (interpreter / wrapper / network \
                 surface — the sandbox does NOT isolate the filesystem or network, so \
                 running it here is no safer); it is never allowed, not even via config"
            ),
        };
    }
    if DENY_RISKY.iter().any(|d| *d == program_lc) {
        return Decision::Deny {
            reason: format!(
                "`{program_lc}` is a high-risk command and the sandbox has no unlock \
                 switch — it does not isolate the filesystem, so running it here gains \
                 nothing over `shell`. Use the `shell` tool with the unlock switch \
                 (setting shell.unlockRisky) instead, which routes it to user approval"
            ),
        };
    }
    let first_arg = rest.first().map(String::as_str).unwrap_or("");
    let allowed = BASE_ALLOW
        .iter()
        .copied()
        .chain(extra_allow.iter().map(|s| s.as_str()))
        .any(|entry| {
            let mut parts = entry.split_ascii_whitespace();
            let entry_prog = parts.next().unwrap_or("").to_ascii_lowercase();
            match parts.next() {
                Some(sub) => program_lc == entry_prog && first_arg.eq_ignore_ascii_case(sub),
                None => program_lc == entry_prog,
            }
        });
    if !allowed {
        return Decision::Deny {
            reason: format!(
                "`{program_lc}` is not whitelisted (built-in: {}{}, \
                 extend via LOCOWRK_SANDBOX_ALLOW)",
                BASE_ALLOW.join(", "),
                if extra_allow.is_empty() {
                    String::new()
                } else {
                    format!(", configured: {}", extra_allow.join(", "))
                }
            ),
        };
    }
    Decision::Allow {
        program,
        args: rest,
    }
}

/// 引号感知切词：空格分隔，单/双引号包住的片段保持原样（引号本身剥掉）。
/// 返回 (首词, 其余参数)；引号不闭合按错误处理。
pub(crate) fn split_command(command: &str) -> Option<(String, Vec<String>)> {
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for ch in command.chars() {
        match quote {
            Some(q) if ch == q => quote = None, // 闭引号
            Some(_) => cur.push(ch),            // 引号内原样收
            None if ch == '\'' || ch == '"' => {
                quote = Some(ch);
                in_word = true;
            }
            None if ch.is_whitespace() => {
                if in_word {
                    tokens.push(std::mem::take(&mut cur));
                    in_word = false;
                }
            }
            None => {
                cur.push(ch);
                in_word = true;
            }
        }
    }
    if quote.is_some() {
        return None; // 引号不闭合 → 判空拒绝（宁拒勿猜）
    }
    if in_word {
        tokens.push(cur);
    }
    let first = tokens.first()?.clone();
    Some((first, tokens.into_iter().skip(1).collect()))
}

#[cfg(test)]
mod tests;
