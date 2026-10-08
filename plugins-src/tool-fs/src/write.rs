//! 写入能力（阶段八⑥第二批，plugins.md §二）：
//! - `fs_write` → `Compensable { undo_tool: "fs_restore" }`：**备份先行**——
//!   先把原内容落进备份目录，再写新内容；备份写失败则整体报错、目标不动；
//!   Revision Guard（可靠性稿 §3，G3）——expected_revision 缺省/null =
//!   新建语义（已存在即拒），给了 revision 必须与当前一致，成功输出带
//!   新 revision 供链式下一次写
//! - `fs_delete` → Irreversible：只删文件，目录拒绝（v1 从简）；同吃
//!   Revision Guard（防误删已被外部改过的文件）
//! - `fs_restore` → Irreversible：undo 契约 `{"call_id", "args", "output"}`，
//!   只读 `output` 里的 {path, backup, existed}；恢复 = 备份 rename 回原位
//!   （备份自消费，重复恢复 no-op，幂等——纪律 7）；输出带恢复后文件的
//!   revision（撤销后续编辑直接续用，守卫不断链）
//!
//! 安全底线：三个工具的路径参数都过 `fsutil` 允许根校验（canonicalize +
//! 前缀）；restore 的 backup 路径额外要求出自数据根 `backups/` 之下
//! （来源必须是本插件产物，防伪造 output 把任意文件搬进工作区）。

use std::path::{Path, PathBuf};

use plugin_api::HostVtable;
use plugin_api::fsutil::{
    arg_expected_revision, backup_original, check_revision, content_revision, effective_roots,
    file_revision, is_not_found, resolve_existing, resolve_for_write, restore_via_rename,
};
use plugin_api::serde_json::{Value, json};

use crate::arg_path;

/// fs_write 内容上限（字节）。与 fs_read 的读取上限同量级，防 runaway 写。
const WRITE_MAX: u64 = 2 * 1024 * 1024;

pub(crate) fn fs_write(args: &Value, host: *const HostVtable) -> Result<Value, String> {
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
            "fs_write content is {} bytes, over the {} limit",
            content.len(),
            WRITE_MAX
        ));
    }
    // 纯校验先于任何文件系统改动：越权目标直接拒绝，不留副作用
    let roots = unsafe { effective_roots(host) }?;
    let resolved = resolve_for_write(Path::new(&path), &roots)?;
    if resolved.is_dir() {
        return Err(format!("fs_write `{}` is a directory", resolved.display()));
    }
    let existed = resolved.is_file();
    let display = resolved.display().to_string();
    // Revision Guard（可靠性稿 §3）：expected_revision 缺省/null = 新建语义
    // ——文件已存在即拒（盲覆盖正是本守卫要断的行为，§3.4）；给了 revision
    // 必须与当前一致，陈旧观察拒绝并指路重读（§3.3）。
    match args.get("expected_revision") {
        None | Some(Value::Null) => {
            if existed {
                return Err(format!(
                    "fs_write rejected: `{display}` already exists but no \
                     `expected_revision` was provided — fs_read it and pass the \
                     `revision` as `expected_revision` to overwrite, or choose a \
                     different path"
                ));
            }
        }
        Some(v) => {
            let expected = v.as_str().ok_or_else(|| {
                "field `expected_revision` must be a string (revision) or null (create new)"
                    .to_string()
            })?;
            let current = file_revision(&resolved)?.ok_or_else(|| {
                format!(
                    "fs_write rejected: `{display}` does not exist (deleted since you \
                     observed it) — create it with `expected_revision`: null"
                )
            })?;
            check_revision("fs_write", &display, expected, &current)?;
        }
    }
    // 备份先行：备份失败 → 整体失败，目标文件保持原样
    let backup = if existed {
        Some(backup_original(&resolved, "tool-fs")?)
    } else {
        None
    };
    if create_dirs {
        if let Some(parent) = resolved.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("fs_write `{}`: create dirs: {e}", resolved.display()))?;
        }
    }
    std::fs::write(&resolved, content).map_err(|e| format!("fs_write `{display}`: {e}"))?;
    Ok(json!({
        "path": display,
        "bytes": content.len(),
        "existed": existed,
        "backup": backup.map(|p| p.display().to_string()),
        "revision": content_revision(content.as_bytes()),
    }))
}

pub(crate) fn fs_delete(args: &Value, host: *const HostVtable) -> Result<Value, String> {
    let path = arg_path(args)?;
    let roots = unsafe { effective_roots(host) }?;
    let resolved = resolve_existing(Path::new(&path), &roots)?;
    if resolved.is_dir() {
        return Err(format!(
            "fs_delete `{}` is a directory (v1 deletes files only)",
            resolved.display()
        ));
    }
    // Revision Guard：删除也要证明「观察未过期」（§3.5 表）——防误删
    // 已被外部改过的文件
    let expected = arg_expected_revision("fs_delete", args)?;
    let display = resolved.display().to_string();
    let current = file_revision(&resolved)?
        .ok_or_else(|| format!("fs_delete rejected: `{display}` no longer exists"))?;
    check_revision("fs_delete", &display, &expected, &current)?;
    std::fs::remove_file(&resolved)
        .map_err(|e| format!("fs_delete `{}`: {e}", resolved.display()))?;
    Ok(json!({ "path": display, "deleted": true }))
}

/// undo 入口。容错两种形态：标准契约 `{"output": {...}}`（内核补偿层传的），
/// 或把 output 字段平铺在 args 顶层（便于手工/直调排查）。
pub(crate) fn fs_restore(args: &Value, host: *const HostVtable) -> Result<Value, String> {
    let output = args.get("output").unwrap_or(args);
    let path = output.get("path").and_then(Value::as_str).ok_or_else(|| {
        "fs_restore: missing `output.path` (pass the original fs_write result)".to_string()
    })?;
    let backup = output.get("backup").and_then(Value::as_str);
    let existed = output
        .get("existed")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    // 目标可能已不存在（撤销"新建"时）→ resolve_for_write 按最深存在祖先校验
    let roots = unsafe { effective_roots(host) }?;
    let target = resolve_for_write(Path::new(path), &roots)?;
    let display = target.display().to_string();
    match backup {
        Some(b) => {
            let bp = Path::new(b);
            if !bp.is_file() {
                // 幂等：备份已被上次恢复消费 → 本次是 no-op（不存在的备份
                // 无数据可搬，伪造它也造不成副作用；存在路径的来源校验在下）
                let revision = file_revision(&target).ok().flatten();
                return Ok(
                    json!({ "path": display, "restored": true, "already": true, "revision": revision }),
                );
            }
            let bp = verify_backup_provenance(bp, host)?;
            restore_via_rename(&bp, &target)?;
            let revision = file_revision(&target).ok().flatten();
            Ok(json!({ "path": display, "restored": true, "already": false, "revision": revision }))
        }
        None => {
            if existed {
                Err("fs_restore: original file existed but the result carries no backup".into())
            } else {
                // 原本不存在 → 撤销 = 删掉新建的文件；NotFound = 已删（幂等）
                match std::fs::remove_file(&target) {
                    Ok(()) => Ok(
                        json!({ "path": display, "restored": true, "removed": true, "revision": null }),
                    ),
                    Err(e) if is_not_found(&e) => Ok(
                        json!({ "path": display, "restored": true, "removed": true, "already": true, "revision": null }),
                    ),
                    Err(e) => Err(format!("fs_restore `{display}`: {e}")),
                }
            }
        }
    }
}

/// 备份来源校验：必须位于数据根 `backups/` 之下且存在（canonicalize 归一
/// 后比前缀）。伪造 output 指向任意文件的一律拒绝。
fn verify_backup_provenance(backup: &Path, host: *const HostVtable) -> Result<PathBuf, String> {
    let resolved = resolve_existing(backup, &unsafe { effective_roots(host) }?)?;
    let vault = plugin_api::fsutil::data_root()?
        .join("backups")
        .canonicalize()
        .map_err(|e| format!("backup vault: {e}"))?;
    if resolved.starts_with(&vault) {
        Ok(resolved)
    } else {
        Err(format!(
            "fs_restore: backup `{}` is not a tool-fs backup (must live under the data root)",
            resolved.display()
        ))
    }
}

#[cfg(test)]
mod tests;
