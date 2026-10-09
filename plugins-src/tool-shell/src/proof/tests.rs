//! 只读证明层单测：安全面（白名单 + 引号内的元字符是数据）/ 危险命令
//! 回归集（§11 坑表「定期用危险命令测试集回归」——结构元字符 / 双引号
//! 展开面 / git 配置覆盖 / 写与外部程序旗标 / 敏感凭据路径 / 白名单外）
//! / 报文出路（指路 shell 审批或单条简单命令）。

use super::{Proof, prove};

/// 断言放行，并取回 program/args 供校验。
fn safe(command: &str) -> (String, Vec<String>) {
    match prove(command) {
        Proof::Safe { program, args } => (program, args),
        other => panic!("`{command}` should be provably read-only, got {other:?}"),
    }
}

/// 断言拒绝（Unsafe/Unknown 都是拒绝），返回报文供出路断言。
fn refused(command: &str) -> String {
    match prove(command) {
        Proof::Safe { .. } => panic!("`{command}` should be refused"),
        Proof::Unsafe { reason } | Proof::Unknown { reason } => reason,
    }
}

#[test]
fn whitelisted_forms_are_provably_readonly() {
    assert_eq!(safe("ls").0, "ls");
    assert_eq!(safe("ls -la src").1, vec!["-la", "src"]);
    assert_eq!(safe("cat file.txt").1, vec!["file.txt"]);
    assert_eq!(safe("grep -rn todo .").1, vec!["-rn", "todo", "."]);
    assert_eq!(safe("git status").1, vec!["status"]);
    assert_eq!(
        safe("git log --oneline -5").1,
        vec!["log", "--oneline", "-5"]
    );
    assert_eq!(safe("git diff HEAD~1").1, vec!["diff", "HEAD~1"]);
    assert_eq!(
        safe("git show --stat abc123").1,
        vec!["show", "--stat", "abc123"]
    );
    // 大小写不敏感（Windows 习惯）；spawn 用原样首词
    assert_eq!(safe("GIT STATUS").0, "GIT");
    assert_eq!(safe("Ls -la").0, "Ls");
}

#[test]
fn quoted_metachars_are_data_not_syntax() {
    // 词法升级的正面收益：引号内的元字符是字面数据，此前整条拒绝
    let (program, args) = safe(r#"grep "a|b" file.txt"#);
    assert_eq!(program, "grep");
    assert_eq!(args, vec!["a|b", "file.txt"]);
    assert_eq!(safe(r#"ls "photo (1).png""#).1, vec!["photo (1).png"]);
    assert_eq!(safe("cat 'plan$(v1).md'").1, vec!["plan$(v1).md"]);
    assert_eq!(safe(r#"grep '(foo|bar)' src"#).1, vec!["(foo|bar)", "src"]);
}

#[test]
fn dangerous_regression_structural_forms() {
    // 设计稿 §10.5 验收原例：`ls $(rm)` 不放行
    for bad in [
        "ls $(rm -rf /)",
        "ls $(rm)",
        "ls `rm`",
        "ls | rm -rf /",
        "cat a; rm b",
        "ls && rm x",
        "ls || cat x",
        "ls > out.txt",
        "ls >> out.txt",
        "cat < /etc/passwd",
        "ls (rm)",
        "git status & git push",
        "cat \"$(x)\"",                  // 双引号内的替换/展开面
        r#"grep "$HOME/.ssh/id_rsa" f"#, // 双引号内的变量展开面
        "cat \"unterminated",
        "cat 'unterminated",
        "ls\nrm -rf /",
    ] {
        let reason = refused(bad);
        assert!(
            reason.contains("not a single simple command"),
            "`{bad}` -> {reason}"
        );
    }
}

#[test]
fn dangerous_regression_git_config_override() {
    // §6.3 条件 6：git -c 能借配置触发外部程序（core.pager 等）
    for bad in [
        r#"git -c core.pager="rm -rf /" status"#,
        "git --git-dir=/x status",
        "git --work-tree=/x log",
        "git --no-pager diff",
        "GIT -C /x STATUS",
    ] {
        let reason = refused(bad);
        assert!(reason.contains("global options"), "`{bad}` -> {reason}");
    }
}

#[test]
fn dangerous_regression_write_and_external_flags() {
    // §6.3 条件 4：写意图 / 外部程序旗标（含带值两种形态）
    for bad in [
        "git diff --output=/tmp/patch",
        "git diff --output patch.txt",
        "git diff --ext-diff",
        "git show --ext-diff abc",
        "git log --textconv",
        "git diff --EXT-DIFF", // 大小写不设防
    ] {
        let reason = refused(bad);
        assert!(reason.contains("not read-only"), "`{bad}` -> {reason}");
    }
}

#[test]
fn dangerous_regression_sensitive_paths() {
    // §6.3 条件 5：敏感凭据路径（豁免面宁严勿松）
    for bad in [
        "cat .env",
        "cat .env.local",
        "ls -la .ssh",
        "cat .ssh/config",
        "grep -r token .ssh",
        "cat credentials.json",
        "grep key .git-credentials",
        "cat id_rsa",
        "cat server.pem",
        "ls ~/.aws",
        "cat backup.pfx",
    ] {
        let reason = refused(bad);
        assert!(
            reason.contains("sensitive path") && reason.contains("`shell` tool"),
            "`{bad}` -> {reason}"
        );
    }
}

#[test]
fn dangerous_regression_whitelist_miss_and_empty() {
    // 白名单外 = 无法证明（Unknown 一律拒绝，§6.6）
    for bad in [
        "git push",
        "git commit -m x",
        "find . -name x",
        "python x.py",
        "whoami",
        "make",
        "cargo test",
    ] {
        let reason = refused(bad);
        assert!(reason.contains("not a read-only command"), "`{bad}`");
    }
    // git 子命令限定照旧
    assert!(refused("git fetch").contains("not a read-only command"));
    for empty in ["", "   "] {
        assert!(refused(empty).contains("empty"));
    }
}

#[test]
fn refuse_messages_teach_next_step() {
    // 结构性：出路 = 单条简单命令（无 shell，复合形态 physically 不可执行）
    let structural = refused("ls $(rm)");
    assert!(structural.contains("pass ONE simple command"));
    // 白名单外：出路 = shell 工具（审批）
    let miss = refused("git push");
    assert!(miss.contains("`shell` tool") && miss.contains("approval"));
}
