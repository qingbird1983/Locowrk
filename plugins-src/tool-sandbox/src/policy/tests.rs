//! 策略层单测：切词 / 元字符 / 两级清单压制 / 白名单与子命令条目 /
//! 启动期冲突检测。与 tool-shell 同源表，用例同步覆盖（拷贝表内容
//! 与真源脱钩的最小保险）。

use super::{Decision, conflicting_allow, decide, split_command};

fn allow(command: &str, extra: &[String]) -> Result<(String, Vec<String>), String> {
    match decide(command, extra) {
        Decision::Allow { program, args } => Ok((program, args)),
        Decision::Deny { reason } => Err(reason),
    }
}

#[test]
fn split_command_keeps_quoted_words_and_rejects_unclosed() {
    assert_eq!(
        split_command("git log \"a b\" 'c d'").unwrap(),
        (
            "git".to_string(),
            vec!["log".to_string(), "a b".to_string(), "c d".to_string()]
        )
    );
    assert!(split_command("git log \"unclosed").is_none());
    assert!(split_command("   ").is_none());
}

#[test]
fn metacharacters_are_rejected_wholesale() {
    for cmd in ["ls | sh", "a & b", "a; b", "cat `x`", "echo $(x)", "a\nb"] {
        let err = allow(cmd, &[]).unwrap_err();
        assert!(err.contains("metacharacters"), "{cmd}: {err}");
    }
}

#[test]
fn hard_lock_wins_over_explicit_allow() {
    for cmd in ["python -V", "cmd /c ver", "curl example.com", "rundll32 x"] {
        let err = allow(cmd, &["python".to_string(), "curl".to_string()]).unwrap_err();
        assert!(err.contains("permanently locked"), "{cmd}: {err}");
    }
}

#[test]
fn risky_is_refused_without_any_unlock_switch() {
    let err = allow("rm -rf tmp", &[]).unwrap_err();
    assert!(err.contains("no unlock switch"), "{err}");
    assert!(err.contains("shell"), "{err}（要引导去 shell 的解锁路径）");
}

#[test]
fn whitelist_and_subcommand_entries() {
    let (prog, _) = allow("git status", &[]).unwrap();
    assert_eq!(prog, "git");
    let err = allow("git push", &[]).unwrap_err();
    assert!(err.contains("not whitelisted"), "{err}");
    allow("ls -la", &[]).unwrap();
}

#[test]
fn explicit_allow_extends_whitelist() {
    let extra = vec!["ping 127.0.0.1".to_string()];
    allow("ping 127.0.0.1", &extra).unwrap();
    // 子命令条目只放行首参匹配的形态
    assert!(allow("ping -n 3 127.0.0.1", &extra).is_err());
}

#[test]
fn conflicting_allow_lists_suppressed_entries() {
    let extra = vec![
        "git status".to_string(),
        "python".to_string(),
        " rm ".to_string(),
    ];
    assert_eq!(conflicting_allow(&extra), vec!["python", " rm "]);
}
