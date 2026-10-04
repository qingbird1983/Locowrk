//! 只读工具（shell_read）的路径校验——只读豁免审批的硬前提（S1）。
//!
//! `cat` / `grep` / `ls` 若无路径约束，就能读允许根外任意文件，绕过
//! fs_* 工具的 `fsutil` 根校验。本模块是 fsutil 守卫的 shell 侧镜像：
//! 允许根**同源**（`plugin_api::fsutil::allowed_roots`，env
//! `LOCOWRK_FS_ROOTS`，缺省 = 数据根），规则宁严勿松——标错拦合法
//! 需求只是多一次审批，标松是越权读：
//! 1. 生效 cwd（显式参数，缺省进程 cwd）canonicalize 后必须在允许根内
//!    ——裸 `ls` 列的就是 cwd，cwd 出根等于整条越权
//! 2. 参数里的路径候选（非 `-` 开头 token、`--flag=值` 的值部分）
//!    resolve 到已存在路径时必须在允许根内（canonicalize 归一 `..` 与
//!    符号链接逃逸；不存在的候选视为模式串/笔误放行——命令自身会报错，
//!    没有读面）
//! 3. `-` 开头但含路径分隔符的 token 一律拒绝（堵 `-f/etc/passwd`
//!    短选项贴值形态，此类合法用例在只读命令里不存在）
//!
//! 任何一条不过 → Err 并引导改用 `shell` 工具（走审批），实现
//! ROADMAP 所谓"降级审批"（降级由模型换工具完成，宿主审批门不变）。

use std::path::{Path, PathBuf};

use plugin_api::HostVtable;
use plugin_api::fsutil::{display_roots, effective_roots};

/// 校验只读命令的路径面。`args` = 切词后的参数（不含程序名），
/// `cwd` = 显式工作目录（已确认存在）。
pub(crate) fn ensure_readonly_scope(
    args: &[String],
    cwd: Option<&Path>,
    host: *const HostVtable,
) -> Result<(), String> {
    // ㉘·B：围栏跟会话（vtable session_context 的 fs_roots 优先，env 回落）
    let roots = unsafe { effective_roots(host) }?;
    let eff_cwd: PathBuf = match cwd {
        Some(dir) => dir.to_path_buf(),
        None => std::env::current_dir()
            .map_err(|e| format!("shell_read: cannot read process cwd: {e}"))?,
    };
    let eff_cwd = eff_cwd
        .canonicalize()
        .map_err(|e| downgrade(&format!("shell_read `cwd` cannot be resolved: {e}")))?;
    if !in_roots(&eff_cwd, &roots) {
        // 自描述（同 fsutil::ensure_inside——2026-10-04 实机走查：旧文案把
        // 归属会话也引去设 env，模型据此误报"零个工作区"）
        return Err(downgrade(&format!(
            "working directory is outside the allowed roots for this call: {} (the \
             fence follows the session's attributed workspace; LOCOWRK_FS_ROOTS is \
             only the fallback)",
            display_roots(&roots)
        )));
    }
    for token in args {
        if let Some(rest) = token.strip_prefix('-') {
            // 规则 3：`-` 开头却带路径分隔符——短选项贴值读文件形态
            if rest.contains('/') || rest.contains('\\') {
                return Err(downgrade(&format!(
                    "flag `{token}` contains a path separator"
                )));
            }
            // `--flag=值`：值部分按路径候选校验
            if let Some(eq) = rest.find('=') {
                check_candidate(&rest[eq + 1..], &eff_cwd, &roots)?;
            }
        } else {
            check_candidate(token, &eff_cwd, &roots)?;
        }
    }
    Ok(())
}

/// 规则 2：候选 resolve 到已存在路径就必须在允许根内。
fn check_candidate(cand: &str, cwd: &Path, roots: &[PathBuf]) -> Result<(), String> {
    if cand.is_empty() {
        return Ok(());
    }
    // 绝对路径 join 时自动替换基路径；相对路径对 cwd 解析（ROADMAP 要求）
    let resolved = cwd.join(cand);
    if let Ok(target) = resolved.canonicalize() {
        if !in_roots(&target, roots) {
            return Err(downgrade(&format!(
                "path argument `{cand}` resolves to `{}` outside the allowed roots",
                target.display()
            )));
        }
    }
    Ok(())
}

fn in_roots(p: &Path, roots: &[PathBuf]) -> bool {
    roots.iter().any(|root| p.starts_with(root))
}

/// 降级文案（ROADMAP：出现允许根外目标 → 降级审批）：错误要能教模型
/// 下一步怎么走。
fn downgrade(what: &str) -> String {
    format!(
        "{what} — refused by the read-only tool; re-run the same command via the \
         `shell` tool (it requires user approval), or adjust the allowed roots \
         (session workspace attribution or LOCOWRK_FS_ROOTS)"
    )
}

#[cfg(test)]
mod tests;
