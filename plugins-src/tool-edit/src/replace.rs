//! edit_replace：old_string → new_string 精确替换。默认强制唯一命中
//! （命中 0 次或多于 1 次都报错——逼模型带上足够上下文，宁错改不误改）。

use std::path::Path;

use plugin_api::HostVtable;
use plugin_api::fsutil::{backup_original, effective_roots, resolve_existing};
use plugin_api::serde_json::{Value, json};

pub(crate) fn edit_replace(args: &Value, host: *const HostVtable) -> Result<Value, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `path`".to_string())?;
    let old = args
        .get("old_string")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `old_string`".to_string())?;
    let new = args
        .get("new_string")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `new_string`".to_string())?;
    let replace_all = args
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if old.is_empty() {
        return Err("edit_replace `old_string` must not be empty".into());
    }
    // ㉘·B 补口（2026-10-04 实机走查发现 v2.56 漏网）：编辑类也是路径类
    // 写面，围栏跟会话（vtable session_context 优先，env 回落）
    // SAFETY: host 是 init 时宿主交付的 'static vtable 指针（EditState 同契约）
    let roots = unsafe { effective_roots(host) }?;
    let resolved = resolve_existing(Path::new(path), &roots)?;
    let display = resolved.display().to_string();
    let content =
        std::fs::read_to_string(&resolved).map_err(|e| format!("edit_replace `{display}`: {e}"))?;

    let count = content.matches(old).count();
    if count == 0 {
        return Err(format!(
            "edit_replace `old_string` not found in `{display}` — re-read the file first"
        ));
    }
    if count > 1 && !replace_all {
        return Err(format!(
            "edit_replace `old_string` matches {count} times in `{display}`; \
             include more surrounding context or pass replace_all=true"
        ));
    }
    // 备份先行：失败则整体报错，目标不动
    let backup = backup_original(&resolved, "tool-edit")?;
    let new_content = if replace_all {
        content.replace(old, new)
    } else {
        content.replacen(old, new, 1)
    };
    std::fs::write(&resolved, &new_content)
        .map_err(|e| format!("edit_replace `{display}`: {e}"))?;
    let edits = if replace_all { count } else { 1 };
    Ok(json!({
        "path": display,
        "bytes": new_content.len(),
        "edits": edits,
        "existed": true,
        "backup": backup.display().to_string(),
    }))
}
