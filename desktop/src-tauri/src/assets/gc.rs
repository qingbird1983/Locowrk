//! 资产 GC（批 D §6.3，三扫描面全扫）：引用面 = sessions/*.jsonl +
//! sessions/*.snapshot（现无磁盘快照，通配兜底）+ sub-transcripts/*.jsonl
//! 三处全量文本粗筛（sha256 hex 子串匹配）→ 孤儿且 mtime 超 gcDays → 删。
//! 不做精确索引（成本 > 收益，7 天宽限兜底误删）。

use std::path::Path;

/// GC 结果（命令层如实上报；Serialize = asset_gc IPC 返回值）。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GcReport {
    pub scanned: usize,
    pub removed: usize,
}

/// 扫引用面 → 删过期孤儿。`assets_root` = `<数据根>/assets`；
/// `sessions_dir` = `<数据根>/sessions`；`sub_dir` = `<数据根>/sub-transcripts`。
pub fn run_gc(assets_root: &Path, sessions_dir: &Path, sub_dir: &Path, gc_days: u32) -> GcReport {
    run_gc_at(assets_root, sessions_dir, sub_dir, gc_days, std::time::SystemTime::now())
}

/// `run_gc` 的时钟注入形态（测试把 now 拨到未来造过期，不碰平台 mtime API）。
pub fn run_gc_at(
    assets_root: &Path,
    sessions_dir: &Path,
    sub_dir: &Path,
    gc_days: u32,
    now: std::time::SystemTime,
) -> GcReport {
    let hay = reference_haystack(&[sessions_dir, sub_dir]);
    let mut report = GcReport {
        scanned: 0,
        removed: 0,
    };
    for dir in ["images", "files"] {
        let Ok(rd) = std::fs::read_dir(assets_root.join(dir)) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            report.scanned += 1;
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if hay.contains(stem) {
                continue; // 被引用（jsonl 全文含 AssetRef id / 旁车 / 子转录）
            }
            if !expired(&path, gc_days, now) {
                continue; // 宽限期内（刚上传 / 走查隔离误伤保护）
            }
            if std::fs::remove_file(&path).is_ok() {
                report.removed += 1;
            }
        }
    }
    report
}

/// 引用面全文拼接（一次性读入——走查机量级 MB 级，可承受）。
fn reference_haystack(dirs: &[&Path]) -> String {
    let mut hay = String::new();
    for dir in dirs {
        let Ok(rd) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            let ext_ok = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e == "jsonl" || e == "snapshot")
                .unwrap_or(false);
            if !ext_ok {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(text) => hay.push_str(&text),
                Err(_) => {
                    // 二进制读不进（理论不会发生）= 按未引用处理，交给宽限期
                    tracing::warn!(path = %path.display(), "GC 引用面文件读取失败");
                }
            }
        }
    }
    hay
}

/// mtime 距 `now` 是否超过 gc_days（0 = 全部立即过期，走查用；`>=` 让
/// 0 天档对刚创建的文件同样成立——as_secs 截断不会漏删）。
fn expired(path: &Path, gc_days: u32, now: std::time::SystemTime) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let Ok(modified) = meta.modified() else {
        return false;
    };
    let Ok(age) = now.duration_since(modified) else {
        return false; // 时钟漂移（未来时间）不删
    };
    age.as_secs() >= u64::from(gc_days) * 24 * 3600
}

#[cfg(test)]
mod tests;
