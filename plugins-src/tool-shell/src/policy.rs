//! shell 白名单策略（plugins.md §二"tool-shell 的安全策略"）：
//! - 默认只允许白名单命令（`ls` / `cat` / `grep` / `git status`）
//! - 危险命令（`rm` / `curl | sh` / `sudo`）**直接拒绝，不进审批流程**
//!   （denylist 永远赢，显式配置也拉不回来）
//! - 想放开 = `LOCOWRK_SHELL_ALLOW` 显式配置（init 时读一次；
//!   `prog` 或 `prog sub` 形态，分号分隔）
//!
//! 无 shell 解析：命令串先过元字符检查（管道 / 命令替换 / 顺序执行
//! 直接拒绝），再引号感知切词，**首词即程序**直接 spawn——不存在
//! `curl | sh` 的注入面，元字符检查是纵深防御。

/// 内置白名单（plugins.md 原文四条）。
pub(crate) const BASE_ALLOW: &[&str] = &["ls", "cat", "grep", "git status"];

/// 硬拒绝清单：即便有人把它加进 LOCOWRK_SHELL_ALLOW 也直接拒绝。
/// 覆盖删除 / 提权 / 远程执行 / 任意代码解释器 / 系统破坏面。
pub(crate) const DENYLIST: &[&str] = &[
    "rm",
    "del",
    "rd",
    "rmdir",
    "srm",
    "shred", //
    "sudo",
    "su",
    "doas",
    "runas", //
    "curl",
    "wget",
    "ftp",
    "scp",
    "ssh", //
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
    "tcsh",
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
    "dd",
    "mkfs",
    "fdisk",
    "diskpart",
    "format",
    "shutdown",
    "reboot",
    "halt", //
    "kill",
    "killall",
    "pkill",
    "taskkill", //
    "chmod",
    "chown",
    "chattr",
    "attrib",
    "icacls",
    "cacls",
    "takeown", //
    "reg",
    "regedit",
    "regsvr32",
    "msiexec",
    "certutil",
    "bitsadmin", //
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

/// 完整判定管线：元字符 → 切词 → denylist → 白名单。
pub(crate) fn decide(command: &str, extra_allow: &[String]) -> Decision {
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
    if DENYLIST.iter().any(|d| *d == program_lc) {
        return Decision::Deny {
            reason: format!(
                "`{program_lc}` is on the permanent denylist (destructive / privilege / \
                 remote-execution surface); it is rejected without approval"
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
