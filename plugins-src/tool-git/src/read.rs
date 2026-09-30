//! 只读 git 操作：status / diff / log（Pure——只读仓库状态，无暂存/提交）。
//! 输出解析基于 porcelain 机器可读格式，不依赖人读格式。

use std::path::Path;

use plugin_api::fsutil::resolve_for_write;
use plugin_api::serde_json::{Value, json};

use crate::proc::{require_ok, run_git};
use crate::{arg_u64, resolve_repo};

pub(crate) fn git_status(args: &Value) -> Result<Value, String> {
    let repo = resolve_repo(args)?;
    let out = run_git(&repo, &["status", "--porcelain=v1", "-b", "--null"])?;
    require_ok(&out, "status")?;
    let (branch, ahead, behind, entries) = parse_status(&out.stdout);
    Ok(json!({
        "repo": repo.display().to_string(),
        "branch": branch,
        "ahead": ahead,
        "behind": behind,
        "clean": entries.is_empty(),
        "entries": entries,
    }))
}

/// `## <branch>...<upstream> [ahead N, behind M]` 或
/// `## No commits yet on <branch>`。无 upstream 时 ahead/behind 为 null。
fn parse_branch_head(head: &str) -> (String, Option<u64>, Option<u64>) {
    let Some(rest) = head.trim().strip_prefix("## ") else {
        return (String::new(), None, None);
    };
    if let Some(branch) = rest.strip_prefix("No commits yet on ") {
        return (branch.to_string(), None, None);
    }
    let mut it = rest.splitn(2, " [");
    let base = it.next().unwrap_or("");
    let branch = base.split("...").next().unwrap_or(base).to_string();
    let (mut ahead, mut behind) = (None, None);
    if let Some(track) = it.next() {
        for seg in track.trim_end_matches(']').split(',') {
            let seg = seg.trim();
            if let Some(n) = seg.strip_prefix("ahead ") {
                ahead = n.parse().ok();
            }
            if let Some(n) = seg.strip_prefix("behind ") {
                behind = n.parse().ok();
            }
        }
    }
    (branch, ahead, behind)
}

/// porcelain --null 输出是**全 NUL 分隔的字段流**（分支头也以 NUL 结尾，
/// 没有换行）：第一个字段是 `## …` 头，其后每个字段是 `XY <path>` 记录；
/// 重命名/复制记录的原始路径是紧随其后的另一个字段（-z 模式没有 ` -> `
/// 箭头形态）。
fn parse_status(text: &str) -> (String, Option<u64>, Option<u64>, Vec<Value>) {
    let mut fields = text.split('\0');
    let (branch, ahead, behind) = parse_branch_head(fields.next().unwrap_or(""));
    let mut entries = Vec::new();
    while let Some(f) = fields.next() {
        if f.is_empty() || f.len() < 4 {
            continue; // 末尾悬挂 NUL / 残缺字段
        }
        let x = f.as_bytes()[0] as char;
        let y = f.as_bytes()[1] as char;
        let path = f[3..].to_string();
        let entry = if matches!(x, 'R' | 'C') {
            let orig = fields.next().unwrap_or("").to_string();
            json!({ "x": x.to_string(), "y": y.to_string(), "path": path, "orig_path": orig })
        } else {
            json!({ "x": x.to_string(), "y": y.to_string(), "path": path })
        };
        entries.push(entry);
    }
    (branch, ahead, behind, entries)
}

pub(crate) fn git_diff(args: &Value) -> Result<Value, String> {
    let repo = resolve_repo(args)?;
    let staged = args.get("staged").and_then(Value::as_bool).unwrap_or(false);
    let path = args.get("path").and_then(Value::as_str);

    let mut argv: Vec<String> = vec!["diff".into(), "--no-color".into()];
    if staged {
        argv.push("--staged".into());
    }
    if let Some(p) = path {
        // 交给 git 的路径必须是仓库内相对路径；绝对/越界路径先校验拒绝
        restrict_path(&repo, p)?;
        argv.push("--".into());
        argv.push(p.to_string());
    }
    let refs: Vec<&str> = argv.iter().map(String::as_str).collect();
    let out = run_git(&repo, &refs)?;
    require_ok(&out, "diff")?;
    Ok(json!({
        "repo": repo.display().to_string(),
        "staged": staged,
        "diff": out.stdout,
        "truncated": out.truncated,
    }))
}

/// diff/log 的 path 限定：canonicalize 后必须仍在仓库内（挡 `../` 逃逸与
/// 允许根内但仓库外的路径）。文件可能不存在（新增文件的 diff 无从谈起，
/// 但保持与 add 相同的祖先回退解析逻辑，语义一致）。
fn restrict_path(repo: &Path, p: &str) -> Result<(), String> {
    let full = if Path::new(p).is_absolute() {
        std::path::PathBuf::from(p)
    } else {
        repo.join(p)
    };
    let roots = plugin_api::fsutil::allowed_roots()?;
    let resolved = resolve_for_write(&full, &roots)?;
    if !resolved.starts_with(repo) {
        return Err(format!("git_diff: path `{p}` is outside the repository"));
    }
    Ok(())
}

/// 字段间 \x1f、记录间 \x1e 的机读 log 格式（人读格式含分隔符歧义）。
const LOG_FORMAT: &str = "--pretty=format:%H%x1f%h%x1f%an%x1f%aI%x1f%s%x1e";

pub(crate) fn git_log(args: &Value) -> Result<Value, String> {
    let repo = resolve_repo(args)?;
    let limit = arg_u64(args, "limit", 20, 100)?;
    let path = args.get("path").and_then(Value::as_str);

    let limit_s = limit.to_string();
    let mut argv: Vec<&str> = vec!["log", "-n", &limit_s, "--date=iso-strict", LOG_FORMAT];
    if let Some(p) = path {
        restrict_path(&repo, p)?;
        argv.push("--");
        argv.push(p);
    }
    let out = run_git(&repo, &argv)?;
    // 空仓库：git log 以非零退出报 "does not have any commits yet"
    if out.code != Some(0) && out.stderr.contains("does not have any commits yet") {
        return Ok(json!({ "commits": [], "empty_repo": true }));
    }
    require_ok(&out, "log")?;

    let commits: Vec<Value> = out
        .stdout
        .split('\u{1e}')
        .filter(|rec| !rec.trim().is_empty())
        .map(|rec| {
            let mut f = rec.split('\u{1f}');
            json!({
                "hash": f.next().unwrap_or(""),
                "short": f.next().unwrap_or(""),
                "author": f.next().unwrap_or(""),
                "date": f.next().unwrap_or(""),
                "subject": f.next().unwrap_or(""),
            })
        })
        .collect();
    Ok(json!({ "commits": commits, "empty_repo": commits.is_empty() }))
}

/// 供 undo 校验 HEAD 复用（write.rs）。
pub(crate) fn head_of(repo: &Path) -> Result<String, String> {
    let out = run_git(repo, &["rev-parse", "HEAD"])?;
    require_ok(&out, "rev-parse")?;
    Ok(out.stdout.trim().to_string())
}
