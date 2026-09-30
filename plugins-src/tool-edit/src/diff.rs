//! edit_diff：严格 unified diff 应用——上下文行必须精确命中，无 fuzz；
//! 任一 hunk 不匹配即整体报错、文件不动（备份都还没写）。v1 规则：
//! 只支持对已存在文件的就地修改（old_start ≥ 1），结尾换行按原文件
//! 是否带 \n 保留（`\ No newline` 标记忽略）。

use std::path::Path;

use plugin_api::fsutil::{allowed_roots, backup_original, resolve_existing};
use plugin_api::serde_json::{Value, json};

use crate::{load_lines, store_lines};

#[derive(Debug, PartialEq, Eq)]
enum Op {
    Context,
    Del,
    Add,
}

#[derive(Debug)]
struct Hunk {
    /// 1-based 旧文件起始行（unified diff 头里的 `-o[,c]`）。
    old_start: u64,
    lines: Vec<(Op, String)>,
}

pub(crate) fn edit_diff(args: &Value) -> Result<Value, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `path`".to_string())?;
    let patch = args
        .get("patch")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `patch`".to_string())?;

    let hunks = parse_patch(patch)?;
    if hunks.is_empty() {
        return Err("edit_diff patch contains no @@ hunks".into());
    }

    let roots = allowed_roots()?;
    let resolved = resolve_existing(Path::new(path), &roots)?;
    let display = resolved.display().to_string();
    let (_, lines, had_nl) = load_lines(&display)?;

    // 先在内存里整段试算（严格匹配），全过了才备份 + 落盘
    let mut out: Vec<String> = Vec::new();
    let mut src = 0usize; // 原文件行游标
    for (i, hunk) in hunks.iter().enumerate() {
        let want = hunk.old_start as usize - 1;
        if want < src {
            return Err(format!(
                "edit_diff hunk #{} overlaps the previous one",
                i + 1
            ));
        }
        if want > lines.len() {
            return Err(format!(
                "edit_diff hunk #{} starts at line {}, beyond the file ({} lines)",
                i + 1,
                hunk.old_start,
                lines.len()
            ));
        }
        out.extend_from_slice(&lines[src..want]);
        src = want;
        for (op, text) in &hunk.lines {
            match op {
                Op::Add => out.push(text.clone()),
                Op::Context | Op::Del => {
                    if lines.get(src).map(String::as_str) != Some(text) {
                        let found = lines.get(src).map(String::as_str).unwrap_or("<eof>");
                        return Err(format!(
                            "edit_diff hunk #{}: expected {op:?} `{text}` at line {}, found `{found}`",
                            i + 1,
                            src + 1
                        ));
                    }
                    if *op == Op::Context {
                        out.push(text.clone());
                    }
                    src += 1;
                }
            }
        }
    }
    out.extend_from_slice(&lines[src..]);

    // 全部匹配 → 备份先行，再落盘
    let backup = backup_original(&resolved, "tool-edit")?;
    let bytes = store_lines(&display, &out, had_nl)?;
    Ok(json!({
        "path": display,
        "bytes": bytes,
        "hunks_applied": hunks.len(),
        "existed": true,
        "backup": backup.display().to_string(),
    }))
}

fn parse_patch(patch: &str) -> Result<Vec<Hunk>, String> {
    let mut hunks = Vec::new();
    let mut cur: Option<Hunk> = None;
    for (no, raw) in patch.lines().enumerate() {
        let line_no = no + 1;
        if let Some(rest) = raw.strip_prefix("@@") {
            // @@ -o[,c] +o[,c] @@ …
            let old_start = rest
                .split(" +")
                .next()
                .and_then(|s| s.trim_start_matches(['-', ' ']).split(',').next())
                .and_then(|s| s.parse::<u64>().ok())
                .ok_or_else(|| format!("edit_diff line {line_no}: malformed @@ header `{raw}`"))?;
            if old_start == 0 {
                return Err(format!(
                    "edit_diff line {line_no}: old_start 0 (file creation) is not supported; \
                     use fs_write for new files"
                ));
            }
            if let Some(h) = cur.take() {
                hunks.push(h);
            }
            cur = Some(Hunk {
                old_start,
                lines: Vec::new(),
            });
            continue;
        }
        let Some(hunk) = cur.as_mut() else {
            // 头部行（---/+++/diff --git/index 等）跳过；其余出现即错
            let is_header = raw.starts_with("---")
                || raw.starts_with("+++")
                || raw.starts_with("diff ")
                || raw.starts_with("index ")
                || raw.trim().is_empty();
            if is_header {
                continue;
            }
            return Err(format!(
                "edit_diff line {line_no}: content outside a hunk (`{raw}`)"
            ));
        };
        let (op, text) = match raw.chars().next() {
            Some(' ') => (Op::Context, raw[1..].to_owned()),
            Some('-') => (Op::Del, raw[1..].to_owned()),
            Some('+') => (Op::Add, raw[1..].to_owned()),
            Some('\\') => continue, // \ No newline at end of file —— v1 忽略
            _ => {
                return Err(format!(
                    "edit_diff line {line_no}: hunk lines must start with ' ', '-' or '+' (`{raw}`)"
                ));
            }
        };
        hunk.lines.push((op, text));
    }
    if let Some(h) = cur.take() {
        hunks.push(h);
    }
    Ok(hunks)
}

#[cfg(test)]
mod tests;
