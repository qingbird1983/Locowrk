//! 写入能力（阶段八⑥第二批，plugins.md §二）：
//! - `fs.write` → `Compensable { undo_tool: "fs.restore" }`：**备份先行**——
//!   先把原内容落进备份目录，再写新内容；备份写失败则整体报错、目标不动
//! - `fs.delete` → Irreversible：只删文件，目录拒绝（v1 从简）
//! - `fs.restore` → Irreversible：undo 契约 `{"call_id", "args", "output"}`，
//!   只读 `output` 里的 {path, backup, existed}；恢复 = 备份 rename 回原位
//!   （备份自消费，重复恢复 no-op，幂等——纪律 7）
//!
//! 安全底线：三个工具的路径参数都过 `fsutil` 允许根校验（canonicalize +
//! 前缀）；restore 的 backup 路径额外要求出自数据根 `backups/` 之下
//! （来源必须是本插件产物，防伪造 output 把任意文件搬进工作区）。

use std::path::{Path, PathBuf};

use plugin_api::fsutil::{
    allowed_roots, backup_original, is_not_found, resolve_existing, resolve_for_write,
    restore_via_rename,
};
use plugin_api::serde_json::{Value, json};

use crate::arg_path;

/// fs.write 内容上限（字节）。与 fs.read 的读取上限同量级，防 runaway 写。
const WRITE_MAX: u64 = 2 * 1024 * 1024;

pub(crate) fn fs_write(args: &Value) -> Result<Value, String> {
    let path = arg_path(args)?;
    let content = args
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `content`".to_string())?;
    let create_dirs = args
        .get("create_dirs")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if content.len() as u64 > WRITE_MAX {
        return Err(format!(
            "fs.write content is {} bytes, over the {} limit",
            content.len(),
            WRITE_MAX
        ));
    }
    // 纯校验先于任何文件系统改动：越权目标直接拒绝，不留副作用
    let roots = allowed_roots()?;
    let resolved = resolve_for_write(Path::new(&path), &roots)?;
    if resolved.is_dir() {
        return Err(format!("fs.write `{}` is a directory", resolved.display()));
    }
    let existed = resolved.is_file();
    // 备份先行：备份失败 → 整体失败，目标文件保持原样
    let backup = if existed {
        Some(backup_original(&resolved, "tool-fs")?)
    } else {
        None
    };
    if create_dirs {
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("fs.write `{}`: create dirs: {e}", resolved.display()))?;
        }
    }
    let display = resolved.display().to_string();
    std::fs::write(&resolved, content).map_err(|e| format!("fs.write `{display}`: {e}"))?;
    Ok(json!({
        "path": display,
        "bytes": content.len(),
        "existed": existed,
        "backup": backup.map(|p| p.display().to_string()),
    }))
}

pub(crate) fn fs_delete(args: &Value) -> Result<Value, String> {
    let path = arg_path(args)?;
    let roots = allowed_roots()?;
    let resolved = resolve_existing(Path::new(&path), &roots)?;
    if resolved.is_dir() {
        return Err(format!(
            "fs.delete `{}` is a directory (v1 deletes files only)",
            resolved.display()
        ));
    }
    std::fs::remove_file(&resolved)
        .map_err(|e| format!("fs.delete `{}`: {e}", resolved.display()))?;
    Ok(json!({ "path": resolved.display().to_string(), "deleted": true }))
}

/// undo 入口。容错两种形态：标准契约 `{"output": {...}}`（内核补偿层传的），
/// 或把 output 字段平铺在 args 顶层（便于手工/直调排查）。
pub(crate) fn fs_restore(args: &Value) -> Result<Value, String> {
    let output = args.get("output").unwrap_or(args);
    let path = output.get("path").and_then(Value::as_str).ok_or_else(|| {
        "fs.restore: missing `output.path` (pass the original fs.write result)".to_string()
    })?;
    let backup = output.get("backup").and_then(Value::as_str);
    let existed = output
        .get("existed")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    // 目标可能已不存在（撤销"新建"时）→ resolve_for_write 按最深存在祖先校验
    let roots = allowed_roots()?;
    let target = resolve_for_write(Path::new(path), &roots)?;
    let display = target.display().to_string();
    match backup {
        Some(b) => {
            let bp = Path::new(b);
            if !bp.is_file() {
                // 幂等：备份已被上次恢复消费 → 本次是 no-op（不存在的备份
                // 无数据可搬，伪造它也造不成副作用；存在路径的来源校验在下）
                return Ok(json!({ "path": display, "restored": true, "already": true }));
            }
            let bp = verify_backup_provenance(bp)?;
            restore_via_rename(&bp, &target)?;
            Ok(json!({ "path": display, "restored": true, "already": false }))
        }
        None => {
            if existed {
                Err("fs.restore: original file existed but the result carries no backup".into())
            } else {
                // 原本不存在 → 撤销 = 删掉新建的文件；NotFound = 已删（幂等）
                match std::fs::remove_file(&target) {
                    Ok(()) => Ok(json!({ "path": display, "restored": true, "removed": true })),
                    Err(e) if is_not_found(&e) => Ok(
                        json!({ "path": display, "restored": true, "removed": true, "already": true }),
                    ),
                    Err(e) => Err(format!("fs.restore `{display}`: {e}")),
                }
            }
        }
    }
}

/// 备份来源校验：必须位于数据根 `backups/` 之下且存在（canonicalize 归一
/// 后比前缀）。伪造 output 指向任意文件的一律拒绝。
fn verify_backup_provenance(backup: &Path) -> Result<PathBuf, String> {
    let resolved = resolve_existing(backup, &allowed_roots()?)?;
    let vault = plugin_api::fsutil::data_root()?
        .join("backups")
        .canonicalize()
        .map_err(|e| format!("backup vault: {e}"))?;
    if resolved.starts_with(&vault) {
        Ok(resolved)
    } else {
        Err(format!(
            "fs.restore: backup `{}` is not a tool-fs backup (must live under the data root)",
            resolved.display()
        ))
    }
}

#[cfg(test)]
mod tests;
