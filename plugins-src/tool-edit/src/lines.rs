//! edit_lines：1-based 闭区间行替换。空 content = 删除该区间。
//! 严格校验：start ≥ 1、end ≥ start、end ≤ 总行数（不越界不追加——
//! 追加语义留给 fs_write，编辑工具只改既有行，宁错不猜）。

use std::path::Path;

use plugin_api::HostVtable;
use plugin_api::fsutil::{
    arg_expected_revision, backup_original, check_revision, content_revision, effective_roots,
    file_revision, resolve_existing,
};
use plugin_api::serde_json::{Value, json};

use crate::{load_lines, store_lines};

pub(crate) fn edit_lines(args: &Value, host: *const HostVtable) -> Result<Value, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `path`".to_string())?;
    let start = args
        .get("start_line")
        .and_then(Value::as_u64)
        .ok_or_else(|| "missing number field `start_line`".to_string())?;
    let end = args
        .get("end_line")
        .and_then(Value::as_u64)
        .ok_or_else(|| "missing number field `end_line`".to_string())?;
    let content = args
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `content`".to_string())?;

    if start == 0 {
        return Err("edit_lines `start_line` is 1-based".into());
    }
    if end < start {
        return Err(format!(
            "edit_lines `end_line` ({end}) must be ≥ `start_line` ({start})"
        ));
    }
    // SAFETY: host 是 init 时宿主交付的 'static vtable 指针（EditState 同契约）
    let roots = unsafe { effective_roots(host) }?;
    let resolved = resolve_existing(Path::new(path), &roots)?;
    let display = resolved.display().to_string();
    let (current, lines, had_nl) = load_lines(&display)?;
    // Revision Guard（可靠性稿 §3，G3）：比对在任何备份/写盘之前
    let expected = arg_expected_revision("edit_lines", args)?;
    check_revision(
        "edit_lines",
        &display,
        &expected,
        &content_revision(current.as_bytes()),
    )?;
    let total = lines.len() as u64;
    if end > total {
        return Err(format!(
            "edit_lines `end_line` ({end}) is beyond the last line ({total}) of `{display}`"
        ));
    }

    // 备份先行：失败则整体报错，目标不动
    let backup = backup_original(&resolved, "tool-edit")?;
    // 换行归一：请求里的 "\r\n" 按行拆掉（写入统一 \n）
    let new_lines: Vec<String> = if content.is_empty() {
        Vec::new()
    } else {
        content
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l).to_owned())
            .collect()
    };
    let replaced = (end - start + 1) as usize;
    // ±N 的真源（后审计队列 Q12·C1 甲案）：本工具按行区间替换，行数是**天然读数**
    // 不需要另算 diff——deletions = 被替换掉的行数，additions = 新内容拆出的行数。
    // 取数必须在 `new_lines` 被 move 进 out 之前。
    let additions = new_lines.len() as i64;
    let mut out: Vec<String> = Vec::with_capacity(lines.len() - replaced + new_lines.len());
    out.extend_from_slice(&lines[..(start - 1) as usize]);
    out.extend(new_lines);
    out.extend_from_slice(&lines[end as usize..]);
    let bytes = store_lines(&display, &out, had_nl)?;
    // 落盘后读回算 revision（与 replace 的内存口径等价——文件刚由本调用写定）
    let revision = file_revision(Path::new(&display))?.unwrap_or_default();
    Ok(json!({
        "path": display,
        "bytes": bytes,
        "replaced": replaced,
        "additions": additions,
        "deletions": replaced as i64,
        "existed": true,
        "backup": backup.display().to_string(),
        "revision": revision,
    }))
}
