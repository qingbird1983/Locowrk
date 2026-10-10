//! 只读证明层（可靠性稿 §6，G4）：从「白名单匹配」升级为「证明只读」。
//!
//! 「命令名字看起来对」不能作为免审批依据——`ls $(rm -rf /)` 的首词
//! 也在白名单里。shell_read 是**完全绕过审批的豁免面**，放行必须以
//! 证明为前提，三段分析（设计稿 §6.2）：
//!
//! 1. **词法**（`lex`）：引号感知切词；未加引号的 `| & ; ( ) < > ` $
//!    与换行是结构性元字符（管道/顺序/重定向/子 shell/替换），双引号
//!    内的 `` $ `` 与反引号也是（shell 语义下的展开面，纵深防御——
//!    宿主无 shell 不展开，但证明层按最坏情形裁决）；单引号内字面。
//! 2. **语法**：词法通过即单条简单命令（结构性失败 = 无法证明）。
//! 3. **语义**（`prove`）：白名单（`policy::READ_ONLY` 固定表）→
//!    git 全局选项（`-c`/`--git-dir` 可借配置触发外部程序，§6.3 条件 6）
//!    → 写意图/外部程序旗标（`--output`/`--ext-diff`/`--textconv`，
//!    条件 4）→ 敏感凭据路径（`.env`/`.ssh`/credentials/密钥材料，
//!    条件 5）。
//!
//! 三态结论（§6.4）：`Safe` = 可证明只读；`Unsafe` = 可证明有副作用；
//! `Unknown` = 无法判断（结构不认识/白名单外）。**Unknown 不等于
//! Safe**，一律拒绝——宁严勿松：标错拦合法需求只是多一次审批（shell
//! 工具降级路径），标松是越权执行（§11 坑表「只读证明太严 = 对的」）。
//!
//! 与既有门的关系（§13 对照）：本层是 shell_read 白名单**之上**的
//! 证明层；`shell` 工具管线自 v2.145 起无命令级禁令（Deny 仅结构性，
//! 其余进审批），路径围栏（paths::ensure_readonly_scope）、外部路径
//! 门全保留不动。
//!
//! 敏感路径启发式与桌面 privacy 门职责不同：privacy 是审批决策表的
//! 「宁漏勿错杀」用户隐私面（`.env` 明确不拦——完全档弹窗不断违背
//! 其存在目的）；本层是免审批豁免面，经济学相反，宁严勿松。

use crate::policy::{EXACT_ARGV, READ_ONLY};

/// 证明结论（§6.4 三态）。Unsafe 与 Unknown 的拒绝路径相同（不放行），
/// 分态只为报文如实：能点名的副作用点名，不能点名的承认看不懂。
pub(crate) enum Proof {
    /// 可证明只读 → 放行执行。
    Safe { program: String, args: Vec<String> },
    /// 可证明有副作用（写旗标 / 外部程序 / 配置覆盖 / 敏感路径）。
    Unsafe { reason: String },
    /// 无法判断（结构元字符 / 引号不闭合 / 白名单外）→ 按 Unsafe 处理。
    Unknown { reason: String },
}

impl std::fmt::Debug for Proof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Proof::Safe { program, args } => {
                write!(f, "Safe({program} {:?})", args)
            }
            Proof::Unsafe { reason } => write!(f, "Unsafe({reason})"),
            Proof::Unknown { reason } => write!(f, "Unknown({reason})"),
        }
    }
}

/// 词法：引号感知切词。返回 token（引号剥掉、内容原样）；结构性元字符
/// / 展开面 / 引号不闭合 = Err（教学报文，指向「单条简单命令」出路）。
/// 反斜杠按普通字符（无 shell 语义下它就是字面值，不作转义解释）。
fn lex(command: &str) -> Result<Vec<String>, String> {
    const STRUCTURAL: &[char] = &['|', '&', ';', '(', ')', '<', '>', '`', '$', '\n', '\r'];
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut quote: Option<char> = None;
    for ch in command.chars() {
        match quote {
            Some(q) => {
                if ch == q {
                    quote = None; // 闭引号
                } else if q == '"' && (ch == '$' || ch == '`') {
                    return Err(structural(&format!(
                        "`{ch}` inside double quotes is an expansion surface"
                    )));
                } else {
                    // 单引号内全字面；双引号内的其余字符也是字面——
                    // 引号里的元字符是数据不是语法
                    cur.push(ch);
                }
            }
            None if ch == '\'' || ch == '"' => {
                quote = Some(ch);
                in_word = true;
            }
            None if STRUCTURAL.contains(&ch) => {
                // 必须先于空白分支：\n/\r 也是 is_whitespace，若先切词，
                // shell 语义下的命令分隔符会被静默拆成两个参数
                return Err(structural(&format!("unquoted `{ch}`")));
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
        return Err(structural("unterminated quote"));
    }
    if in_word {
        tokens.push(cur);
    }
    Ok(tokens)
}

/// 结构性拒绝的教学报文（§3.3：错误信息直接告诉 AI 下一步做什么）。
fn structural(what: &str) -> String {
    format!(
        "{what}: not a single simple command — read-only proof cannot be established, \
         and Locowrk spawns programs without a shell, so pipes / redirection / command \
         substitution / subshells cannot execute at all; pass ONE simple command \
         (compute any substitution yourself first and use the literal value)"
    )
}

/// 语义分析：词法 → 白名单 → git 全局选项 → 写/外部旗标 → 敏感路径。
/// 任何一环证明不了 → Unsafe/Unknown（报文带出路）。
pub(crate) fn prove(command: &str) -> Proof {
    let tokens = match lex(command) {
        Ok(t) => t,
        Err(reason) => return Proof::Unknown { reason },
    };
    let Some((program, args)) = tokens.split_first() else {
        return Proof::Unknown {
            reason: "shell command is empty".into(),
        };
    };
    let program = program.to_owned();
    let program_lc = program.to_ascii_lowercase();
    let first_arg = args.first().map(String::as_str).unwrap_or("");
    // 精确 argv 表先于一切语义检查：整条命令逐 token 相等即证毕（表内容
    // 本身零副作用——版本打印 / pwd），git 全局选项守卫对它天然无接触
    let exact = EXACT_ARGV.iter().any(|argv| {
        argv[0].eq_ignore_ascii_case(&program_lc)
            && args.len() == argv.len() - 1
            && args
                .iter()
                .zip(argv[1..].iter())
                .all(|(a, e)| a.eq_ignore_ascii_case(e))
    });
    if exact {
        return Proof::Safe {
            program,
            args: args.to_vec(),
        };
    }
    // git 全局选项（§6.3 条件 6）先于白名单子命令匹配——`-c`/`--git-dir`
    // 打头的首参本来就不在子命令表里，顺序反了会被泛化白名单报文盖掉
    if program_lc == "git" && first_arg.starts_with('-') {
        return Proof::Unsafe {
            reason: format!(
                "`git` global options (`{first_arg}`) can change behaviour or launch \
                 external programs via config (e.g. `core.pager`) — not provably \
                 read-only; use plain `git <subcommand> …` with no global options"
            ),
        };
    }
    let whitelisted = READ_ONLY.iter().any(|(prog, subs)| {
        program_lc == *prog
            && (subs.is_empty() || subs.iter().any(|s| first_arg.eq_ignore_ascii_case(s)))
    });
    if !whitelisted {
        return Proof::Unknown {
            reason: format!(
                "`{program_lc}` is not a read-only command (allowed: ls, cat, grep, \
                 git status|log|diff|show, pwd, git --version, node -v/--version, \
                 python[3] --version); use the `shell` tool instead — it runs the \
                 same command but requires user approval"
            ),
        };
    }
    if let Some(flag) = args.iter().find(|t| write_flag(t)) {
        return Proof::Unsafe {
            reason: format!(
                "`{flag}` writes a file or can launch external programs (diff.external / \
                 textconv drivers) — not read-only; drop the flag, or re-run via the \
                 `shell` tool (requires user approval)"
            ),
        };
    }
    if let Some(token) = args.iter().find(|t| sensitive_path(t).is_some()) {
        return Proof::Unsafe {
            reason: format!(
                "`{token}` looks like a credential / sensitive path (.env, .ssh, key \
                 material, credentials) — reading it needs explicit user approval; \
                 re-run via the `shell` tool (the user will be asked)"
            ),
        };
    }
    Proof::Safe {
        program,
        args: args.to_vec(),
    }
}

/// 写意图 / 外部程序旗标（§6.3 条件 4）。带值形态（`--output=x`）与
/// 空格带值形态（`--output x` 的前 token）都拦。
fn write_flag(token: &str) -> bool {
    const FLAGS: &[&str] = &["--output", "--ext-diff", "--textconv"];
    let lc = token.to_ascii_lowercase();
    FLAGS
        .iter()
        .any(|f| lc == *f || lc.strip_prefix(*f).is_some_and(|r| r.starts_with('=')))
}

/// 敏感凭据路径（§6.3 条件 5）：只看路径候选（非 `-` 开头 token、
/// `--flag=值` 的值部分）。宁严勿松——`grep "\.env" file` 这类把敏感名
/// 当检索词的合法用例也会被拦（多一次审批），换免审批面零越权读。
fn sensitive_path(token: &str) -> Option<&'static str> {
    const MARKERS: &[&str] = &[
        // 环境与凭据目录/文件（含主目录凭据老三样与密钥材料后缀）
        ".env",
        ".ssh",
        ".aws",
        ".gnupg",
        ".kube",
        ".netrc",
        ".git-credentials",
        "credentials",
        "id_rsa",
        "id_ed25519",
        "id_ecdsa",
        "id_dsa",
        ".pem",
        ".pfx",
        ".p12",
        ".kdbx", //
    ];
    let cand = match token.strip_prefix("--") {
        Some(rest) => rest.split_once('=').map(|(_, v)| v)?,
        None if token.starts_with('-') => return None,
        None => token,
    };
    let lc = cand.to_ascii_lowercase();
    MARKERS.iter().find(|m| lc.contains(*m)).copied()
}

#[cfg(test)]
mod tests;
