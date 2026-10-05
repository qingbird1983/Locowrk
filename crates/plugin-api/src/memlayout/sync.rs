//! 同步机械件（F2，《Memory File-ization Design》§3）：文件 ↔ 索引双向
//! 同步的底层——per-file mtime 标记（`.sync/`）、原子写（防并发撕裂）、
//! frontmatter 解析（reconcile 读回）与**冲突备份**（写路径检测到未合并
//! 的人工编辑时保留双方）。插件侧与宿主侧共用同一份实现。

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::{FactEntry, fact_rel_path, refresh_index};

/// 标记目录（memory 根下，隐藏目录，不进任何 scope 扫描）。
const SYNC_DIR: &str = ".sync";

/// 同文件并发写时的临时名去重（进程内计数；两写者同内容，撕裂即消除）。
static TMP_SEQ: AtomicU64 = AtomicU64::new(0);

/// rel 路径 → 标记路径：`<root>/.sync/<rel 去扩展名>.ts`。
fn marker_path(root: &Path, rel: &Path) -> PathBuf {
    root.join(SYNC_DIR).join(rel.with_extension("ts"))
}

/// 读标记（本模块上次落笔时的文件 mtime_ms）。缺 / 坏 = None。
pub fn read_marker(root: &Path, rel: &Path) -> Option<i64> {
    let text = std::fs::read_to_string(marker_path(root, rel)).ok()?;
    text.trim().parse::<i64>().ok()
}

/// 落 / 更新标记（原子写）。
pub fn write_marker(root: &Path, rel: &Path, ms: i64) -> Result<(), String> {
    let path = marker_path(root, rel);
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("memory 标记目录创建失败 {}: {e}", dir.display()))?;
    }
    atomic_write(&path, &ms.to_string())
}

/// 删标记（随文件删除；缺 = 幂等）。
pub fn remove_marker(root: &Path, rel: &Path) {
    let _ = std::fs::remove_file(marker_path(root, rel));
}

/// 文件 mtime（毫秒）。不可得（不存在等）= None。
pub fn mtime_ms(path: &Path) -> Option<i64> {
    let t = std::fs::metadata(path).ok()?.modified().ok()?;
    Some(t.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64)
}

/// 原子写：临时文件（唯一名，同目录）+ rename——并发写不再产生撕裂的
/// md / 索引 / 标记（Windows `rename` 覆盖既有目标）。
pub fn atomic_write(path: &Path, data: &str) -> Result<(), String> {
    let n = TMP_SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = PathBuf::from(format!("{}.tmp{}", path.display(), n));
    std::fs::write(&tmp, data).map_err(|e| format!("memory 临时写失败 {}: {e}", tmp.display()))?;
    match std::fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!("memory 原子写失败 {}: {e}", path.display()))
        }
    }
}

/// 解析出的一条事实（reconcile 读回 / 冲突备份；字段缺 = 人的文件里没写）。
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedFact {
    /// frontmatter `name`（权威键位身份；无 frontmatter = None → 调用方
    /// 回落文件名主干）。
    pub name: Option<String>,
    /// metadata.workspace 原值（F1 落盘身份；无 = None → 调用方按目录反查）。
    pub workspace: Option<String>,
    /// metadata.source（无 = None）。
    pub source: Option<String>,
    /// metadata.updated（无 / 坏 = None）。
    pub updated_at: Option<i64>,
    /// 正文（闭合 `---` 之后，剥一个紧随空行；保留到行粒度）。
    pub body: String,
}

/// YAML 双引号标量反转义（与渲染侧 `yaml_str` 互逆；裸标量原样）。
pub(crate) fn yaml_unquote(v: &str) -> String {
    let v = v.trim();
    let un = v
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(v);
    un.replace("\\\"", "\"").replace("\\\\", "\\")
}

/// 解析单文件（宽松：无 frontmatter = 全文正文；未闭合块按无 frontmatter）。
/// 身份以 frontmatter `name` 为准（设计 §3——文件名只是键位的净空编码）。
pub fn parse_fact(text: &str) -> ParsedFact {
    let lines: Vec<&str> = text.split('\n').collect();
    if lines.first().copied() == Some("---") {
        let mut name = None;
        let mut workspace = None;
        let mut source = None;
        let mut updated_at = None;
        let mut in_meta = false;
        for (i, line) in lines.iter().enumerate().skip(1) {
            if line.trim_end() == "---" {
                let mut rest = &lines[i + 1..];
                if rest.first().copied() == Some("") {
                    rest = &rest[1..]; // 渲染侧 `---` 后的空行分隔
                }
                if rest.last().copied() == Some("") {
                    rest = &rest[..rest.len() - 1]; // 渲染侧文件末尾的单个换行
                }
                return ParsedFact {
                    name,
                    workspace,
                    source,
                    updated_at,
                    body: rest.join("\n"),
                };
            }
            if let Some(v) = line.strip_prefix("name: ") {
                name = Some(yaml_unquote(v));
            } else if line.trim_end() == "metadata:" {
                in_meta = true;
            } else if in_meta {
                if let Some(v) = line.strip_prefix("  workspace: ") {
                    workspace = Some(yaml_unquote(v));
                } else if let Some(v) = line.strip_prefix("  source: ") {
                    source = Some(yaml_unquote(v));
                } else if let Some(v) = line.strip_prefix("  updated: ") {
                    updated_at = v.trim().parse::<i64>().ok();
                } else if !line.starts_with(' ') && !line.trim().is_empty() {
                    in_meta = false;
                }
            }
        }
    }
    ParsedFact {
        name: None,
        workspace: None,
        source: None,
        updated_at: None,
        body: text.to_string(),
    }
}

/// 写路径冲突保护（设计 §3 同步纪律「保留双方 + 新者为准」）：目标文件带
/// **未合并的人工编辑**（标记在而 mtime 已变，内容 ≠ 本次渲染）→ 先把现
/// 文件另存 `<key> · 人工编辑备份 <millis>` 备份键（frontmatter name 即
/// reconcile 的吸纳身份），再落新值——模型写入发生在后 = 时间戳新者占
/// 主键，人的版本以备份行存活。返回备份键（无冲突 = None）。
pub fn stash_human_edit(
    root: &Path,
    rel: &Path,
    incoming: &FactEntry<'_>,
) -> Result<Option<String>, String> {
    let Some(expected) = read_marker(root, rel) else {
        return Ok(None); // 无基线（F1 时代文件 / 人新建）——reconcile 内容比对兜底
    };
    let Some(actual) = mtime_ms(&root.join(rel)) else {
        return Ok(None); // 文件不在 = 无可保护
    };
    if actual == expected {
        return Ok(None); // 自上次落笔未动
    }
    let Ok(current) = std::fs::read_to_string(root.join(rel)) else {
        return Ok(None);
    };
    if current == incoming.render() {
        return Ok(None); // 内容相同（打开没改）
    }
    let parsed = parse_fact(&current);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let backup_key = format!("{} · 人工编辑备份 {}", incoming.key, millis);
    let backup_rel = fact_rel_path(incoming.workspace, &backup_key);
    let backup_path = root.join(&backup_rel);
    if let Some(dir) = backup_path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("memory 备份目录创建失败 {}: {e}", dir.display()))?;
    }
    let backup = FactEntry {
        workspace: incoming.workspace,
        key: &backup_key,
        value: parsed.body.trim_end(),
        tags: "",
        source: "human",
        updated_at: millis / 1000,
    };
    atomic_write(&backup_path, &backup.render())?;
    if let Some(ms) = mtime_ms(&backup_path) {
        let _ = write_marker(root, &backup_rel, ms);
    }
    refresh_index(root, incoming.workspace)?;
    Ok(Some(backup_key))
}
