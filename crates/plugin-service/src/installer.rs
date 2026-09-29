//! 安装器（DESIGN.md §8 installer.rs）：包校验 + 解压落位。
//!
//! 本地市场的"下载"就是目录读取，"解压"就是目录复制。落位布局与
//! 多版本回退直接相关：`install_root/{id}/{version}/`（§11.3）。
//! Windows 上已加载的 DLL 无法改名/移动，因此更新不走 staging + rename，
//! 直接复制进最终版本目录（新版本目录不与在用文件冲突，§11.5）。

use std::path::{Path, PathBuf};

use semver::Version;

use crate::market::{PackageManifest, artifact_file_name};

/// 从市场目录读取并校验一个包：manifest 自洽 + 平台库文件在位。
/// 返回（源目录, 市场元数据）。
pub fn verify(src_dir: &Path, id: &str, version: &Version) -> Result<PackageManifest, String> {
    if !src_dir.is_dir() {
        return Err(format!("package dir {} missing", src_dir.display()));
    }
    let artifact = src_dir.join(artifact_file_name(id));
    if !artifact.is_file() {
        return Err(format!("artifact {} missing", artifact.display()));
    }
    let path = src_dir.join("manifest.json");
    let raw =
        std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let m: PackageManifest =
        serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
    if m.id != id || m.version != version.to_string() {
        return Err(format!(
            "manifest mismatch: declared {}@{}, expected {id}@{version}",
            m.id, m.version
        ));
    }
    Ok(m)
}

/// 把包目录复制到 `install_root/{id}/{version}/`，返回目标目录。
/// 目标已存在（重装/更新重试）则覆盖同名文件，不先删目录——
/// Windows 上在用 DLL 删不掉，覆盖写是唯一安全动作。
pub fn extract(
    src_dir: &Path,
    install_root: &Path,
    id: &str,
    version: &Version,
) -> Result<PathBuf, String> {
    verify(src_dir, id, version)?;
    let dst = install_root.join(id).join(version.to_string());
    std::fs::create_dir_all(&dst).map_err(|e| format!("create_dir_all {}: {e}", dst.display()))?;
    copy_dir_contents(src_dir, &dst)?;
    Ok(dst)
}

fn copy_dir_contents(src: &Path, dst: &Path) -> Result<(), String> {
    let entries = std::fs::read_dir(src).map_err(|e| format!("read_dir {}: {e}", src.display()))?;
    for entry in entries.flatten() {
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if from.is_dir() {
            std::fs::create_dir_all(&to)
                .map_err(|e| format!("create_dir_all {}: {e}", to.display()))?;
            copy_dir_contents(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|e| format!("copy {} -> {}: {e}", from.display(), to.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
