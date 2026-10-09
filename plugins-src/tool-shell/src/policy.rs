//! shell 白名单策略（plugins.md §二 + 清单《Setting and Constrained
//! Optimization.md》§2/§3 三级分类）：
//! - **只读级**（`shell_read` 专用，豁免审批）：固定表 `ls` / `cat` /
//!   `grep` / `git status|log|diff|show`——之上另有只读证明层
//!   （proof.rs，词法/语法/语义三态，G4），路径校验见 paths.rs
//! - **审批级**（`shell` 工具）：内置白名单 `ls` / `cat` / `grep` /
//!   `git status`，`LOCOWRK_SHELL_ALLOW` 显式扩展（`prog` / `prog sub`）
//! - **高危清单**：rm / sudo / chmod / dd 等"用户能判断后果"的破坏面——
//!   默认拒绝，解锁开关（`LOCOWRK_SHELL_UNLOCK_RISK`，桌面端由设置
//!   `shell.unlockRisky` 在插件加载前注入）开启后放行——shell 工具本身
//!   Irreversible，放行即进审批
//! - **硬锁清单**：解释器 / 包装（间接执行）/ 网络 / 代码执行面
//!   LOLBin——用户判断不了后果，永不放行、不给开关（清单 §3"致命"级）
//!
//! denylist 永远赢：白名单（含显式配置）撞上两级清单一律被压制，
//! 启动期经 conflicting_allow() 列出并 warn（清单 §2.4，不静默）。
//!
//! 无 shell 解析：`shell` 管线命令串先过元字符检查（管道 / 命令替换 /
//! 顺序执行直接拒绝），再引号感知切词，**首词即程序**直接 spawn——
//! 不存在 `curl | sh` 的注入面，元字符检查是纵深防御；`shell_read`
//! 的结构检查由证明层按引号位置精确裁决（引号内元字符是数据）。

/// 只读白名单（`shell_read` 第一道门）：`prog` + 可选子命令集合；子命令
/// 集合为空 = 只按程序名放行。固定内容，不吃 LOCOWRK_SHELL_ALLOW。
/// 只读证明层（proof.rs）在表之上做词法/语义证明——表内不等于放行。
pub(crate) const READ_ONLY: &[(&str, &[&str])] = &[
    ("ls", &[]),
    ("cat", &[]),
    ("grep", &[]),
    ("git", &["status", "log", "diff", "show"]),
];

/// 审批工具（shell）内置白名单（plugins.md 原文四条）。
pub(crate) const BASE_ALLOW: &[&str] = &["ls", "cat", "grep", "git status"];

/// 硬锁清单：解释器 / 包装 / 网络 / 代码执行面。即便加进
/// LOCOWRK_SHELL_ALLOW 也直接拒绝，且没有解锁开关（清单 §2.3/§3：
/// 这几类能间接执行任意代码，用户判断不了后果）。
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

/// 高危清单：rm / sudo / chmod / dd 等"用户能判断后果"的破坏面
/// （清单 §2.7/§3 裁决，含 §2.3 磁盘/系统类 mount/chroot/systemctl）。
/// 默认拒绝；解锁开关开启后放行进审批（shell 工具 Irreversible，
/// 放行即审批），硬锁类不受开关影响。
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
    std::env::var("LOCOWRK_SHELL_ALLOW")
        .unwrap_or_default()
        .split([';', ','])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}

/// 解锁开关（init 读一次）：`1` / `true` / `yes` / `on`。桌面端由设置
/// `shell.unlockRisky` 在插件加载前写 env；改动后重启生效。
pub(crate) fn unlock_risky_from_env() -> bool {
    matches!(
        std::env::var("LOCOWRK_SHELL_UNLOCK_RISK").as_deref(),
        Ok("1") | Ok("true") | Ok("yes") | Ok("on")
    )
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

/// 审批工具（shell）的完整判定管线：元字符 → 切词 → 硬锁 → 高危 →
/// 白名单。
pub(crate) fn decide(command: &str, extra_allow: &[String], unlock_risky: bool) -> Decision {
    if command.contains(METACHARS) {
        return Decision::Deny {
            reason: "shell command contains shell metacharacters (| & ; < > ` $ or newline); \
                     pass a single simple command — pipes and command substitution are not allowed"
                .into(),
        };
    }
    let Some((program, rest)) = split_command(command) else {
        return Decision::Deny {
            reason: "shell command is empty".into(),
        };
    };
    let program_lc = program.to_ascii_lowercase();
    if DENY_HARD.iter().any(|d| *d == program_lc) {
        return Decision::Deny {
            reason: format!(
                "`{program_lc}` is permanently locked (interpreter / wrapper / network \
                 surface — arbitrary code execution the user cannot judge); it is never \
                 allowed, not even via config"
            ),
        };
    }
    if DENY_RISKY.iter().any(|d| *d == program_lc) {
        if !unlock_risky {
            return Decision::Deny {
                reason: format!(
                    "`{program_lc}` is a high-risk command, refused by default; enable the \
                     unlock switch (setting shell.unlockRisky / LOCOWRK_SHELL_UNLOCK_RISK, \
                     takes effect after restart) to send it to user approval instead"
                ),
            };
        }
        // 解锁后放行：shell 工具 Irreversible，放行即进审批（清单 §2.7：
        // "走审批而非直接放行"由工具副作用保证，策略层不重复拦截）。
        return Decision::Allow {
            program,
            args: rest,
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
                 extend via LOCOWRK_SHELL_ALLOW)",
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
/// 返回 (首词, 其余参数)；引号不闭合按错误处理。`shell` 审批管线的切词
/// 入口（元字符检查在前，此处只管切）；`shell_read` 走 proof::lex
/// （按引号位置精确裁决元字符）。
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
