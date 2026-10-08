//! 只读三件套：fs_read / fs_list / fs_grep（全 Pure——随便调，回退免费）。
//!
//! 安全底线：路径参数先过 `fsutil::resolve_existing`（canonicalize + 允许根
//! 前缀校验）；目录遍历**跳过符号链接**（read_dir 的 file_type 不跟随，
//! 后续 read 才跟随——不跳过的话根内符号链接就是逃逸口）。

use std::io::Read;
use std::path::PathBuf;

use plugin_api::HostVtable;
use plugin_api::fsutil::{content_revision, effective_roots, resolve_existing};
use plugin_api::serde_json::{Value, json};

use crate::{arg_path, arg_u64};

/// 单文件读取缺省/上限（字节）。上限防一次调用灌爆模型上下文。
const READ_DEFAULT: u64 = 32 * 1024;
const READ_MAX: u64 = 1024 * 1024;
/// grep 默认/上限：结果条数、递归深度、扫描文件数、单行摘录长度。
const GREP_RESULTS_DEFAULT: u64 = 50;
const GREP_RESULTS_MAX: u64 = 200;
const GREP_DEPTH_DEFAULT: u64 = 3;
const GREP_DEPTH_MAX: u64 = 8;
const GREP_FILE_MAX: u64 = 1024 * 1024;
const GREP_LINE_EXCERPT: usize = 200;

pub(crate) fn fs_read(args: &Value, host: *const HostVtable) -> Result<Value, String> {
    let path = arg_path(args)?;
    let max = arg_u64(args, "max_bytes", READ_DEFAULT, READ_MAX)?;
    let roots = unsafe { effective_roots(host) }?;
    let resolved = resolve_existing(std::path::Path::new(&path), &roots)?;
    let display = resolved.display().to_string();
    let meta = std::fs::metadata(&resolved).map_err(|e| format!("fs_read `{display}`: {e}"))?;
    if meta.is_dir() {
        return Err(format!("fs_read `{display}` is a directory (use fs_list)"));
    }
    let mut file =
        std::fs::File::open(&resolved).map_err(|e| format!("fs_read `{display}`: {e}"))?;
    // 全量读入（Revision Guard，可靠性稿 §3）：revision 必须覆盖完整内容、
    // 与返回的截断窗口解耦——模型拿到的 revision 是它没读到的部分也算数的
    // 承诺。内存口径与 backup_original（写侧全量备份）一致。
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)
        .map_err(|e| format!("fs_read `{display}`: {e}"))?;
    let revision = content_revision(&buf);
    let truncated = buf.len() as u64 > max;
    buf.truncate(max as usize);
    Ok(json!({
        "path": display,
        "size": meta.len(),
        "truncated": truncated,
        "revision": revision,
        "content": String::from_utf8_lossy(&buf).as_ref(),
    }))
}

pub(crate) fn fs_list(args: &Value, host: *const HostVtable) -> Result<Value, String> {
    let path = arg_path(args)?;
    let roots = unsafe { effective_roots(host) }?;
    let resolved = resolve_existing(std::path::Path::new(&path), &roots)?;
    let display = resolved.display().to_string();
    let rd = std::fs::read_dir(&resolved).map_err(|e| format!("fs_list `{display}`: {e}"))?;
    let mut entries: Vec<(bool, Value)> = rd
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let ft = e.file_type().ok()?;
            // 符号链接不跟随：kind=link，后续读它会被 resolve_existing 挡下
            let kind = if ft.is_symlink() {
                "link"
            } else if ft.is_dir() {
                "dir"
            } else if ft.is_file() {
                "file"
            } else {
                "other"
            };
            let size = e.metadata().ok().map(|m| json!(m.len()));
            Some((
                ft.is_dir(),
                json!({ "name": name, "kind": kind, "size": size }),
            ))
        })
        .collect();
    // 目录在前，其余按名字排（确定性输出，模型好读）
    entries.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| a.1["name"].as_str().cmp(&b.1["name"].as_str()))
    });
    let entries: Vec<Value> = entries.into_iter().map(|(_, v)| v).collect();
    Ok(json!({ "path": display, "entries": entries }))
}

pub(crate) fn fs_grep(args: &Value, host: *const HostVtable) -> Result<Value, String> {
    let path = arg_path(args)?;
    let query = args
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `query`".to_string())?
        .to_lowercase();
    if query.is_empty() {
        return Err("fs_grep `query` must not be empty".into());
    }
    let max_results =
        arg_u64(args, "max_results", GREP_RESULTS_DEFAULT, GREP_RESULTS_MAX)? as usize;
    let max_depth = arg_u64(args, "max_depth", GREP_DEPTH_DEFAULT, GREP_DEPTH_MAX)?;

    let roots = unsafe { effective_roots(host) }?;
    let base = resolve_existing(std::path::Path::new(&path), &roots)?;
    let mut matches: Vec<Value> = Vec::new();
    let mut truncated = false;
    let mut scanned = 0u64;
    // 迭代栈代替递归：带深度预算的目录遍历；符号链接不入栈（逃逸口）
    let mut stack: Vec<(PathBuf, u64)> = vec![(base, 0)];
    'outer: while let Some((cur, depth)) = stack.pop() {
        let Ok(meta) = std::fs::metadata(&cur) else {
            continue;
        };
        if meta.is_dir() {
            if depth >= max_depth {
                continue;
            }
            let Ok(rd) = std::fs::read_dir(&cur) else {
                continue;
            };
            let mut children: Vec<PathBuf> = rd
                .flatten()
                .filter(|e| e.file_type().map(|ft| !ft.is_symlink()).unwrap_or(false))
                .map(|e| e.path())
                .collect();
            children.sort();
            // 倒序入栈：pop 出来仍是名字序
            for child in children.into_iter().rev() {
                stack.push((child, depth + 1));
            }
            continue;
        }
        if meta.len() > GREP_FILE_MAX {
            continue; // 大文件跳过（二进制/日志），不计入 scanned
        }
        scanned += 1;
        let Ok(bytes) = std::fs::read(&cur) else {
            continue;
        };
        for (i, line) in String::from_utf8_lossy(&bytes).lines().enumerate() {
            if line.to_lowercase().contains(&query) {
                let mut text = line.to_string();
                if text.chars().count() > GREP_LINE_EXCERPT {
                    text = text.chars().take(GREP_LINE_EXCERPT).collect();
                }
                matches.push(json!({
                    "file": cur.display().to_string(),
                    "line": i + 1,
                    "text": text,
                }));
                if matches.len() >= max_results {
                    truncated = true;
                    break 'outer;
                }
            }
        }
    }
    Ok(json!({
        "query": query,
        "matches": matches,
        "truncated": truncated,
        "scanned": scanned,
    }))
}

#[cfg(test)]
mod tests;
