//! 变更类 git 操作：commit（Compensable，输出携带新提交哈希供 undo）/
//! push（Irreversible）/ undo_commit（Irreversible，HEAD 校验后 soft reset）。

use std::path::{Path, PathBuf};

use plugin_api::fsutil::{allowed_roots, resolve_for_write};
use plugin_api::serde_json::{Value, json};

use crate::proc::{require_ok, run_git};
use crate::read::head_of;
use crate::{resolve_repo, resolve_repo_str};

pub(crate) fn git_commit(args: &Value) -> Result<Value, String> {
    let repo = resolve_repo(args)?;
    let message = args
        .get("message")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `message`".to_string())?;
    if message.trim().is_empty() {
        return Err("git_commit: `message` must not be empty".into());
    }
    let add_all = args
        .get("add_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let add: Vec<String> = args
        .get("add")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if add_all && !add.is_empty() {
        return Err("git_commit: use either `add` or `add_all`, not both".into());
    }
    if !add_all && add.is_empty() {
        return Err(
            "git_commit: nothing to stage — pass `add` (paths) or `add_all`=true".to_string(),
        );
    }

    if add_all {
        let out = run_git(&repo, &["add", "-A"])?;
        require_ok(&out, "add -A")?;
    } else {
        // 纯校验先于任何 git 动作：相对路径按仓库根解析（不是宿主 CWD），
        // canonicalize 后必须仍在仓库内（允许根由 resolve_for_write 把关）
        for p in &add {
            ensure_path_in_repo(&repo, p)?;
        }
        // `--` 之后的路径不会被当成选项（挡 `-oXxx` 式 argv 注入）
        let mut argv: Vec<&str> = vec!["add", "--"];
        argv.extend(add.iter().map(String::as_str));
        let out = run_git(&repo, &argv)?;
        require_ok(&out, "add")?;
    }

    let out = run_git(&repo, &["commit", "-m", message])?;
    require_ok(&out, "commit")?;
    let commit = head_of(&repo)?;
    let short = commit.get(..7).unwrap_or(&commit).to_string();
    Ok(json!({
        "repo": repo.display().to_string(),
        "commit": commit,
        "short": short,
        "message": message,
    }))
}

/// add 路径校验：相对路径按仓库根拼绝对，canonicalize（最深存在祖先回退，
/// 新文件也允许）后必须仍在仓库内。
fn ensure_path_in_repo(repo: &Path, p: &str) -> Result<PathBuf, String> {
    let full = if Path::new(p).is_absolute() {
        PathBuf::from(p)
    } else {
        repo.join(p)
    };
    let roots = allowed_roots()?;
    let resolved = resolve_for_write(&full, &roots)?;
    if !resolved.starts_with(repo) {
        return Err(format!("git_commit: path `{p}` is outside the repository"));
    }
    Ok(resolved)
}

/// 远端 / 分支名白名单：挡 argv 选项注入（`-oProxyCommand` 等）与 refname
/// 里的歧义字符。git 自身规则更细，这里取交集的保守子集。
fn valid_refname(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(['-', '+'])
        && !name.contains("..")
        && !name.ends_with(".lock")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '-' | '_'))
}

pub(crate) fn git_push(args: &Value) -> Result<Value, String> {
    let repo = resolve_repo(args)?;
    let remote = args
        .get("remote")
        .and_then(Value::as_str)
        .unwrap_or("origin");
    let branch = args
        .get("branch")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `branch`".to_string())?;
    for (field, name) in [("remote", remote), ("branch", branch)] {
        if !valid_refname(name) {
            return Err(format!(
                "git_push: `{field}` `{name}` contains unsupported characters"
            ));
        }
    }
    let out = run_git(&repo, &["push", remote, branch])?;
    require_ok(&out, "push")?;
    Ok(json!({
        "pushed": true,
        "remote": remote,
        "branch": branch,
        "output": out.stdout,
    }))
}

/// undo 入口（契约 `{"call_id","args","output"}`）：只读 output.{repo, commit}。
/// HEAD 已移动（之后又有提交 / reset / checkout）时拒绝——soft reset 只会
/// 回退"当前 HEAD 的上一条"，对错位提交执行就是改写错误的历史。
pub(crate) fn git_undo_commit(args: &Value) -> Result<Value, String> {
    let output = args.get("output").unwrap_or(args);
    let repo_str = output.get("repo").and_then(Value::as_str).ok_or_else(|| {
        "git_undo_commit: missing `output.repo` (pass the original git_commit result)".to_string()
    })?;
    let commit = output
        .get("commit")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            "git_undo_commit: missing `output.commit` (pass the original git_commit result)"
                .to_string()
        })?;
    let repo = resolve_repo_str(repo_str)?;

    let head = head_of(&repo)?;
    if head != commit {
        return Err(format!(
            "git_undo_commit: HEAD has moved ({head} != {commit}); manual intervention required"
        ));
    }
    let out = run_git(&repo, &["reset", "--soft", "HEAD~1"])?;
    require_ok(&out, "reset")?;
    Ok(json!({ "undone": true, "commit": commit }))
}
