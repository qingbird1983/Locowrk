//! 记忆文件化布局（F1，《Memory File-ization Design v1.1》）：记忆的人类
//! 层 = `<数据根>/memory/` 下的 Markdown——人可查可读可写，编辑是权威；
//! memory.sqlite3 保留为机器索引与召回快路径（WorkBuddy / ZCode 同款
//! 双层）。本模块只放**布局与格式**（目录命名、文件名安全、frontmatter
//! 渲染、写删与 MEMORY.md 索引刷新）——插件侧（memory-sqlite）与宿主侧
//! （desktop memory / memory_files）共用同一份实现，同一键位必落同一
//! 文件。写入边界（宁严勿松）：全部落点都在数据根 `memory/` 之内。
//!
//! 布局（设计 §2.1）：
//! ```text
//! memory/
//! ├── MEMORY.md                        # （预留给全局视图，本模块不写）
//! ├── global/<key>.md                  # 全局记忆（'' 键位：用户偏好）
//! └── projects/<可读名-短哈希>/
//!     ├── MEMORY.md                    # 该项目事实索引（一文件一行）
//!     └── facts/<key>.md               # 项目事实
//! ```

use std::path::{Path, PathBuf};

use crate::fsutil;

mod sync;

pub use sync::{ParsedFact, mtime_ms, parse_fact, read_marker, write_marker};

/// md 根目录名（数据根下）。
const DIR_NAME: &str = "memory";
/// 总开关 env：宿主在 settings `memory.files = false` 时注入 `"0"`；
/// 插件**每次写前读取**（与 fsutil allowed_roots 同款读 env 热换，保存
/// 即生效免重启）。env 缺省未设 = 开。
const ENV_FILES: &str = "LOCOWRK_MEMORY_FILES";

/// 总开关判定（纯函数，可测）：`Some("0")` = 关，其余（含 None）= 开。
fn enabled_from(v: Option<&str>) -> bool {
    v != Some("0")
}

/// 当前进程的文件化总开关（插件侧调用；宿主侧以 settings 为准）。
pub fn files_enabled() -> bool {
    enabled_from(std::env::var(ENV_FILES).ok().as_deref())
}

/// md 根目录（`<数据根>/memory`）。数据根不可用 = Err（调用方按降级处理）。
pub fn memory_root() -> Result<PathBuf, String> {
    Ok(fsutil::data_root()?.join(DIR_NAME))
}

/// fnv-1a 64 位（无依赖、跨运行稳定；与 memory-tree scope_dir 同算法）。
pub fn fnv1a64(data: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in data {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// 项目 md 文件夹名 = `<可读名>-<fnv 短哈希>`（设计 §8 已裁定②）：可读
/// 名取路径末段（安全化 + 截 40 字符），短哈希 = 完整路径 fnv1a 高 32 位
/// hex8——同名目录不冲突、异名同路径恒等。与 memory-tree 的 scope_dir
/// **编码独立**（那是机器投影目录，这是人读目录）。
pub fn project_dir_name(workspace: &str) -> String {
    let readable = Path::new(workspace)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(workspace);
    let safe: String = readable
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let head: String = safe
        .trim_matches(|c: char| c == '.' || c == '_')
        .chars()
        .take(40)
        .collect();
    let head = if head.is_empty() {
        "project".to_string()
    } else {
        head
    };
    format!(
        "{head}-{:08x}",
        (fnv1a64(workspace.as_bytes()) >> 32) as u32
    )
}

/// key → 文件名主干：保留 `[A-Za-z0-9._-]`、其余换 `_`，端点 `.` 剥除，
/// Windows 保留设备名（con/prn/aux/nul/comN/lptN）前缀 `_` 防创建失败。
/// 碰撞可能但罕见；权威身份在 frontmatter `name`（F2 reconcile 按它读回）。
pub fn key_file_stem(key: &str) -> String {
    let safe: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let trimmed = safe.trim_matches('.');
    let stem: String = if trimmed.is_empty() {
        "_".into()
    } else {
        trimmed.to_string()
    };
    let base = stem.split('.').next().unwrap_or("").to_ascii_lowercase();
    let reserved = ["con", "prn", "aux", "nul"];
    let com_lpt_digit = (base.len() == 4)
        && (base.starts_with("com") || base.starts_with("lpt"))
        && base[3..].chars().all(|c| c.is_ascii_digit());
    let looks_reserved = reserved.contains(&base.as_str()) || com_lpt_digit;
    if looks_reserved {
        format!("_{stem}")
    } else {
        stem
    }
}

/// 键位 → 相对路径：全局 `''` = `global/<stem>.md`；项目 =
/// `projects/<dir>/facts/<stem>.md`。
pub fn fact_rel_path(workspace: &str, key: &str) -> PathBuf {
    let file = format!("{}.md", key_file_stem(key));
    if workspace.is_empty() {
        PathBuf::from("global").join(file)
    } else {
        PathBuf::from("projects")
            .join(project_dir_name(workspace))
            .join("facts")
            .join(file)
    }
}

/// 一条事实（双写与导出的渲染输入）。
pub struct FactEntry<'a> {
    pub workspace: &'a str,
    pub key: &'a str,
    pub value: &'a str,
    pub tags: &'a str,
    /// explicit | extracted（zcode 式 frontmatter 之外 Locowrk 自有溯源）。
    pub source: &'a str,
    pub updated_at: i64,
}

/// YAML 双引号标量（手渲染足够：值域受控，换行先被 one_line 折掉）。
fn yaml_str(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

/// 单行化 + 截断（description / 索引 hook 同源）。
pub fn one_line(s: &str, max_chars: usize) -> String {
    let flat: String = s
        .chars()
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let trimmed = flat.trim();
    if trimmed.chars().count() > max_chars {
        format!("{}…", trimmed.chars().take(max_chars).collect::<String>())
    } else {
        trimmed.to_string()
    }
}

impl FactEntry<'_> {
    /// description = tags，空则正文首行（索引 hook 与 frontmatter 同源）。
    pub fn description(&self) -> String {
        if self.tags.trim().is_empty() {
            one_line(self.value, 80)
        } else {
            one_line(self.tags, 80)
        }
    }

    /// 单文件全文：frontmatter（name / description / metadata）+ 正文。
    /// workspace 与 name 落盘 = F2 reconcile 的身份读回依据。
    pub fn render(&self) -> String {
        format!(
            "---\nname: {}\ndescription: {}\nmetadata:\n  type: fact\n  source: {}\n  workspace: {}\n  updated: {}\n---\n\n{}\n",
            yaml_str(self.key),
            yaml_str(&self.description()),
            yaml_str(if self.source.is_empty() {
                "explicit"
            } else {
                self.source
            }),
            yaml_str(self.workspace),
            self.updated_at,
            self.value
        )
    }
}

/// 键位目录与索引文件：全局 = `global/`，项目 = `projects/<dir>/facts/`
/// （索引坐在项目文件夹根）。
fn scope_dirs(root: &Path, workspace: &str) -> (PathBuf, PathBuf) {
    if workspace.is_empty() {
        let dir = root.join("global");
        (dir.clone(), dir.join("MEMORY.md"))
    } else {
        let dir = root.join("projects").join(project_dir_name(workspace));
        (dir.join("facts"), dir.join("MEMORY.md"))
    }
}

/// 从 frontmatter 读回 description（索引 hook 用；无 = None）。
fn parse_description(text: &str) -> Option<String> {
    for line in text.lines().skip(1) {
        if line.trim_end() == "---" {
            break;
        }
        if let Some(v) = line.strip_prefix("description: ") {
            return Some(sync::yaml_unquote(v));
        }
    }
    None
}

/// 重建一个键位的 MEMORY.md 索引（一文件一行，文件名稳定序）。facts
/// 目录不存在时也会创建（空索引自描述）。
pub fn refresh_index(root: &Path, workspace: &str) -> Result<(), String> {
    let (dir, index_path) = scope_dirs(root, workspace);
    let mut lines: Vec<String> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for f in entries.flatten() {
            let name = f.file_name();
            let Some(stem) = name.to_str().and_then(|s| s.strip_suffix(".md")) else {
                continue;
            };
            if stem == "MEMORY" {
                continue;
            }
            let desc = std::fs::read(f.path())
                .ok()
                .and_then(|b| {
                    parse_description(&String::from_utf8_lossy(&b)).map(|d| one_line(&d, 80))
                })
                .unwrap_or_default();
            lines.push(format!("- [{stem}]({stem}.md) — {desc}"));
        }
    }
    lines.sort();
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("memory 目录创建失败 {}: {e}", dir.display()))?;
    sync::atomic_write(
        &index_path,
        &format!("# 记忆索引\n\n{}\n", lines.join("\n")),
    )
    .map_err(|e| format!("memory 索引写入失败 {}: {e}", index_path.display()))
}

/// 写一条事实 md（目录惰性建 + **冲突备份** + 刷新该键位索引）。幂等
/// 覆盖——调用方保证 sqlite 已是真源。返回值 = 文件路径（日志/测试用）。
pub fn write_fact(root: &Path, e: &FactEntry) -> Result<PathBuf, String> {
    let rel = fact_rel_path(e.workspace, e.key);
    let path = root.join(&rel);
    let dir = path
        .parent()
        .ok_or_else(|| format!("memory 路径异常: {}", rel.display()))?;
    std::fs::create_dir_all(dir)
        .map_err(|err| format!("memory 目录创建失败 {}: {err}", dir.display()))?;
    // F2 冲突保护：文件带未合并的人工编辑 → 先备份再写（保留双方）
    sync::stash_human_edit(root, &rel, e)?;
    sync::atomic_write(&path, &e.render())
        .map_err(|err| format!("memory md 写入失败 {}: {err}", path.display()))?;
    if let Some(ms) = sync::mtime_ms(&path) {
        let _ = sync::write_marker(root, &rel, ms);
    }
    refresh_index(root, e.workspace)?;
    Ok(path)
}

/// 删一条事实 md + 删标记 + 刷新索引。文件不存在 = Ok(false)（幂等）。
pub fn delete_fact(root: &Path, workspace: &str, key: &str) -> Result<bool, String> {
    let rel = fact_rel_path(workspace, key);
    let path = root.join(&rel);
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if fsutil::is_not_found(&e) => {
            sync::remove_marker(root, &rel);
            return Ok(false);
        }
        Err(e) => return Err(format!("memory md 删除失败 {}: {e}", path.display())),
    }
    sync::remove_marker(root, &rel);
    refresh_index(root, workspace)?;
    Ok(true)
}

#[cfg(test)]
mod tests;
