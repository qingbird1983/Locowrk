//! 文件类工具插件的安全基座（Rust 便利层，**不进 C ABI 面**——只给
//! Rust 插件作者复用，符号约定与 describe 契约不变）。
//!
//! 安全底线（用户 2026-09-29 指定）：
//! - 所有路径参数必须 canonicalize 之后校验 `starts_with(允许根)`，
//!   `../` 与符号链接逃逸都被 canonicalize 归一后挡在根外（fail-closed）
//! - 允许根 = `LOCOWRK_FS_ROOTS`（`;` 分隔，桌面端/CLI 启动时设为
//!   工作区目录），未设置时**只放行数据根**（备份目录在数据根下，
//!   fs_restore 天然可达）；默认最小暴露，放开必须显式配置
//! - 写入前备份原内容到数据根 `backups/<plugin>/`，undo 按路径自寻

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// 数据根：`LOCOWRK_DATA_DIR` 优先，否则 HOME 下 `.Locowrk`（desktop
/// data_root.rs 同规则；插件不依赖宿主，此处为权威共享实现）。
pub fn data_root() -> Result<PathBuf, String> {
    if let Ok(dir) = std::env::var("LOCOWRK_DATA_DIR") {
        if !dir.is_empty() {
            return Ok(PathBuf::from(dir));
        }
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .map_err(|_| "cannot locate home dir (USERPROFILE/HOME)".to_string())?;
    Ok(PathBuf::from(home).join(".Locowrk"))
}

/// 允许根（canonicalize 后）：`LOCOWRK_FS_ROOTS` 按 `;` 切分；未设置或
/// 为空 = 只放行数据根。任何一个配置根不存在都直接报错（配置错误要可见，
/// 不静默缩小攻击面也不静默扩大）。数据根不存在则创建（备份/恢复本来
/// 就要求它在），并同样 canonicalize——与 resolve_* 的结果同前缀
/// （Windows 下 canonicalize 产生 `\\?\` verbatim 前缀，必须两边一致）。
pub fn allowed_roots() -> Result<Vec<PathBuf>, String> {
    let extra = std::env::var("LOCOWRK_FS_ROOTS").unwrap_or_default();
    let data = data_root()?;
    if !data.exists() {
        std::fs::create_dir_all(&data)
            .map_err(|e| format!("data root `{}`: {e}", data.display()))?;
    }
    let mut roots = vec![
        data.canonicalize()
            .map_err(|e| format!("data root `{}`: {e}", data.display()))?,
    ];
    for raw in extra.split(';') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let root = Path::new(raw)
            .canonicalize()
            .map_err(|e| format!("LOCOWRK_FS_ROOTS entry `{raw}`: {e}"))?;
        roots.push(root);
    }
    Ok(roots)
}

/// 调用期围栏根（㉘·B）：经 vtable `session_context` 读宿主下发的
/// `fs_roots`（归属会话 = `[数据根, 项目工作区]`，宿主侧已 canonicalize）。
/// 非调用期 / headless / 空表 → `None`，调用方回落 [`allowed_roots`]。
/// 单个根 canonicalize 失败即剔除（宿主给的本就是规范路径，失败 = 根已被
/// 移除；宁严勿松——少一个根不会扩大攻击面）。
///
/// # Safety
/// `host` 必须是 init 时宿主交付的合法指针（与插件侧日志同契约）。
pub unsafe fn context_roots(host: *const crate::HostVtable) -> Option<Vec<PathBuf>> {
    // SAFETY: host 是 init 时宿主交付的 'static vtable 指针（函数 Safety 契约）
    let vt = unsafe { host.as_ref()? };
    let mut buf = [0u8; 4096];
    // SAFETY: 同上——vtable 指针合法即可调用其 extern "C" 成员
    let n = unsafe { vt.session_context(&mut buf) }.ok()?;
    let ctx = crate::ffi::CallContext::from_json(&buf[..n])?;
    if ctx.fs_roots.is_empty() {
        return None;
    }
    let roots: Vec<PathBuf> = ctx
        .fs_roots
        .iter()
        .filter_map(|r| Path::new(r).canonicalize().ok())
        .collect();
    (!roots.is_empty()).then_some(roots)
}

/// 文件围栏的唯一取根入口（㉘·B 起）：调用期根优先（[`context_roots`]，
/// 围栏跟会话），回落 env 全局（[`allowed_roots`]，未归属 / headless 语义
/// 与 ㉘·B 之前完全一致）。
///
/// # Safety
/// `host` 必须是 init 时宿主交付的合法指针（透传给 [`context_roots`]）。
pub unsafe fn effective_roots(host: *const crate::HostVtable) -> Result<Vec<PathBuf>, String> {
    // SAFETY: Safety 契约由调用方保证（同本函数 doc）
    if let Some(roots) = unsafe { context_roots(host) } {
        return Ok(roots);
    }
    allowed_roots()
}

/// 已存在路径的规范路径（读 / 列 / 删 / 恢复目标）：canonicalize 会解析
/// 符号链接与 `..`——解析结果落到允许根外即拒绝。目标不存在时**裁决理由
/// 保持一致**：按最深存在祖先归一后判根，根外仍报 outside（拒绝必须可
/// 识别，不能长得像读失败——2026-09 冒烟测试两度把该报错误读成"守卫被
/// 绕过"），根内才报原始 NotFound（真找不到）。
pub fn resolve_existing(path: &Path, roots: &[PathBuf]) -> Result<PathBuf, String> {
    match path.canonicalize() {
        Ok(resolved) => ensure_inside(&resolved, roots),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            resolve_for_write(path, roots)?;
            Err(format!("`{}`: {e}", path.display()))
        }
        Err(e) => Err(format!("`{}`: {e}", path.display())),
    }
}

/// 写目标（可能尚不存在）：从最深存在祖先 canonicalize 后拼回其余组件，
/// 再校验前缀——**纯校验，不动文件系统**（create_dirs 由调用方在校验
/// 通过后自己执行，避免"先建目录后发现越权"的外泄副作用）。
pub fn resolve_for_write(path: &Path, roots: &[PathBuf]) -> Result<PathBuf, String> {
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = path;
    let resolved = loop {
        match cur.canonicalize() {
            Ok(c) => break c,
            Err(_) => {
                let name = cur.file_name().ok_or_else(|| {
                    format!("`{}`: cannot resolve any existing ancestor", path.display())
                })?;
                missing.push(name.to_os_string());
                match cur.parent() {
                    Some(p) => cur = p,
                    None => {
                        return Err(format!(
                            "`{}`: cannot resolve any existing ancestor",
                            path.display()
                        ));
                    }
                }
            }
        }
    };
    let mut resolved = resolved;
    // 缺失尾部里的 `..`/`.` 必须归一后再判前缀——否则 `root\a\missing\..\..\..\x`
    // 这类路径词法上仍在根内（骗过 starts_with），OS 解析 `..` 后却落在根外
    for comp in missing.iter().rev() {
        let name = comp.to_string_lossy();
        if name == ".." {
            resolved.pop();
        } else if name != "." {
            resolved.push(comp);
        }
    }
    ensure_inside(&resolved, roots)
}

/// 把已写入的数据文件备份进 `<数据根>/backups/<plugin>/<纳秒>-<pid>/<文件名>`，
/// 返回备份绝对路径（进 ToolResult.output，undo 时按它找回）。
pub fn backup_original(target: &Path, plugin: &str) -> Result<PathBuf, String> {
    let bytes =
        std::fs::read(target).map_err(|e| format!("backup read `{}`: {e}", target.display()))?;
    let dir = backup_dir(plugin)?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("backup create dir: {e}"))?;
    let name = target
        .file_name()
        .map(std::ffi::OsStr::to_os_string)
        .unwrap_or_else(|| "file".into());
    let dest = dir.join(name);
    std::fs::write(&dest, bytes).map_err(|e| format!("backup write `{}`: {e}", dest.display()))?;
    Ok(dest)
}

fn backup_dir(plugin: &str) -> Result<PathBuf, String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    Ok(data_root()?
        .join("backups")
        .join(plugin)
        .join(format!("{nanos}-{}", std::process::id())))
}

fn ensure_inside(resolved: &Path, roots: &[PathBuf]) -> Result<PathBuf, String> {
    if roots.iter().any(|r| resolved.starts_with(r)) {
        Ok(resolved.to_path_buf())
    } else {
        // 拒绝必须自描述（2026-10-04 实机走查：内嵌 Agent 试遍围栏外路径后
        // 误报"零个工作区"——旧文案只教人去设 env）：报错列出本次调用实际
        // 生效的根。㉘·C 起围栏 = 全局声明根（数据根 + 设置里的工作区根，
        // 与权限档无关），文案照实说并指路设置加根。
        Err(format!(
            "path `{}` is outside the allowed roots for this call: {} — allowed \
             roots are the workspace roots declared in settings (fsRoots) plus \
             the data root; add the missing root in settings to browse it",
            resolved.display(),
            display_roots(roots)
        ))
    }
}

/// 围栏根的展示形（报错自描述用）：去 Windows verbatim `\\?\` 前缀
/// （canonicalize 产物，原样出现会干扰阅读），逐根反引号括起。
pub fn display_roots(roots: &[PathBuf]) -> String {
    let items: Vec<String> = roots
        .iter()
        .map(|r| {
            let s = r.to_string_lossy();
            format!("`{}`", s.strip_prefix(r"\\?\").unwrap_or(&s))
        })
        .collect();
    format!("[{}]", items.join(", "))
}

/// rename 恢复的共享原语（同卷原子；跨卷 EXDEV 回退 copy + remove）。
/// 目标父目录可能已被删过，先兜底创建。
pub fn restore_via_rename(backup: &Path, target: &Path) -> Result<(), String> {
    if let Some(parent) = target.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::rename(backup, target).is_ok() {
        return Ok(());
    }
    std::fs::copy(backup, target).map_err(|e| format!("restore -> `{}`: {e}", target.display()))?;
    let _ = std::fs::remove_file(backup);
    Ok(())
}

/// 便于 undo 工具区分"文件确实不在"与真 IO 错误。
pub fn is_not_found(e: &std::io::Error) -> bool {
    matches!(e.kind(), ErrorKind::NotFound)
}

#[cfg(test)]
mod tests;
