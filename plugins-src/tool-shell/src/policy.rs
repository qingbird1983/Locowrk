//! shell 政策层（《Permission Model Redesign (ZCode Alignment) v1》，
//! 对齐 ZCode 权限模型）：
//! - **只读级**（`shell_read` 专用，豁免审批）：READ_ONLY 固定表 +
//!   EXACT_ARGV 精确表——之上是只读证明层（proof.rs，词法/语法/语义
//!   三段分析），路径校验见 paths.rs。免审批面宁严勿松口径不变。
//! - **审批级**（`shell` 工具）：**无命令级禁令**——Deny 仅剩结构性
//!   拒绝（元字符 / 空命令 / 引号未闭合），其余一切命令放行进审批卡
//!   （工具 Irreversible，放行即审批）。解释器 / 高危 / 网络类的安全
//!   语义由审批卡 + 粘性规则（桌面 permission_rules）承载——审批管
//!   「怎么用」，不做「能不能用」级禁令（ZCode HIGH_RISK_ROOT_COMMANDS
//!   同款哲学；2026-10-10 用户裁定，v2.26 时代的三级分类硬锁退役）。
//!
//! 元字符检查保留为结构性拒绝（唯一幸存的 deny）：宿主无 shell，管道
//! / 命令替换 / 顺序执行 physically 不可执行，整条拒绝是教学而非防御
//! ——报文指路「单条简单命令」。
//!
//! 无 shell 解析：`shell` 管线命令串先过元字符检查，再引号感知切词，
//! **首词即程序**直接 spawn；`shell_read` 的结构检查由证明层按引号
//! 位置精确裁决（引号内元字符是数据）。

/// 只读白名单（`shell_read` 第一道门）：`prog` + 可选子命令集合；子命令
/// 集合为空 = 只按程序名放行。固定内容。只读证明层（proof.rs）在表之
/// 上做词法/语义证明——表内不等于放行。
pub(crate) const READ_ONLY: &[(&str, &[&str])] = &[
    ("ls", &[]),
    ("cat", &[]),
    ("grep", &[]),
    ("git", &["status", "log", "diff", "show"]),
];

/// 精确 argv 只读表（ZCode READONLY_EXACT_ARGV_COMMANDS 同款 + pwd +
/// git --version）：整条命令逐 token 相等才放行——`node --version` 免审，
/// `node --version foo` / `git --version --git-dir=/x` 不在表内照常拒绝
/// （宁严勿松）。解释器的版本检查判断不了任何后果，与 ZCode 一致列为
/// 只读；git 的 `--version` 走精确表而非子命令表，全局选项守卫零例外。
pub(crate) const EXACT_ARGV: &[&[&str]] = &[
    &["pwd"],
    &["git", "--version"],
    &["node", "-v"],
    &["node", "--version"],
    &["python", "--version"],
    &["python3", "--version"],
];

/// 元字符：管道 / 顺序执行 / 重定向 / 命令替换 / 变量展开。出现在命令串
/// 里就整条拒绝（不进审批）。
const METACHARS: &[char] = &['|', '&', ';', '<', '>', '`', '$', '\n', '\r'];

/// 策略判定（含原因，错误信息要能教模型怎么改）。
pub(crate) enum Decision {
    Allow { program: String, args: Vec<String> },
    Deny { reason: String },
}

/// 退役 env 幻影化提示（v2.145：`LOCOWRK_SHELL_ALLOW` /
/// `LOCOWRK_SHELL_UNLOCK_RISK` 不再有任何效果——前者白名单职责由只读
/// 面与粘性规则承接，后者的「放行进审批」成为缺省路径）。设置即启动
/// warn 一次，不静默。
pub(crate) fn retired_env_warns() -> Vec<String> {
    let mut warns = Vec::new();
    if std::env::var_os("LOCOWRK_SHELL_ALLOW").is_some() {
        warns.push(
            "tool-shell: LOCOWRK_SHELL_ALLOW is retired and ignored since v2.145 — \
             commands no longer need a whitelist; they go to user approval, and \
             sticky project rules (本项目始终允许) replace config extensions"
                .into(),
        );
    }
    if std::env::var_os("LOCOWRK_SHELL_UNLOCK_RISK").is_some() {
        warns.push(
            "tool-shell: LOCOWRK_SHELL_UNLOCK_RISK is retired and ignored since \
             v2.145 — there is no command denylist left to unlock; every command \
             goes to user approval by default"
                .into(),
        );
    }
    warns
}

/// 切词失败形态：引号不闭合 vs 空命令。两条报文各指各的出路（错误信息
/// 要教模型下一步），合并成一条会让模型去查空串而不是补引号。
#[derive(Debug)]
pub(crate) enum SplitError {
    UnclosedQuote,
    Empty,
}

/// 审批工具（shell）的完整判定管线：元字符 → 切词 → 放行进审批。
pub(crate) fn decide(command: &str) -> Decision {
    if command.contains(METACHARS) {
        return Decision::Deny {
            reason: "shell command contains shell metacharacters (| & ; < > ` $ or newline); \
                     pass a single simple command — pipes and command substitution are not allowed"
                .into(),
        };
    }
    let (program, rest) = match split_command(command) {
        Ok(pair) => pair,
        // 两条拒绝报文分开给（错误信息要教模型下一步）：空命令 → 给一个
        // 命令词；引号不闭合 → 补上右引号。合并成「command is empty」会
        // 让模型去查空串而不是补引号。
        Err(SplitError::UnclosedQuote) => {
            return Decision::Deny {
                reason: "shell command has an unclosed quote — close the matching ' or \" \
                         and retry (quotes are stripped after closing)"
                    .into(),
            };
        }
        Err(SplitError::Empty) => {
            return Decision::Deny {
                reason: "shell command is empty — pass at least one command word".into(),
            };
        }
    };
    // 无命令级禁令：策略层只保证「单条简单命令、可切词」。副作用语义
    // 交审批门（工具 Irreversible）+ 粘性规则承载。
    Decision::Allow {
        program,
        args: rest,
    }
}

/// 引号感知切词：空格分隔，单/双引号包住的片段保持原样（引号本身剥掉）。
/// 返回 (首词, 其余参数)；引号不闭合与空命令是两种失败（报文不同，
/// 见 [`SplitError`]）。`shell` 审批管线的切词入口（元字符检查在前，
/// 此处只管切）；`shell_read` 走 proof::lex（按引号位置精确裁决元字符）。
pub(crate) fn split_command(command: &str) -> Result<(String, Vec<String>), SplitError> {
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
        // 引号不闭合 = 结构错误，绝不猜后半段当参数（宁拒勿猜）
        return Err(SplitError::UnclosedQuote);
    }
    if in_word {
        tokens.push(cur);
    }
    if tokens.is_empty() {
        return Err(SplitError::Empty);
    }
    let first = tokens[0].clone();
    Ok((first, tokens.into_iter().skip(1).collect()))
}

#[cfg(test)]
mod tests;
