//! 记忆文件化布局（F1，《Memory File-ization Design v1.1》）：记忆的唯一真源 =
//! `<数据根>/memory/` 下的 Markdown——人可查可读可写，编辑即权威写入。
//! 本模块只放**布局与格式**（目录命名、文件名安全、frontmatter 渲染、写删与
//! MEMORY.md 索引刷新）——插件侧（memory-sqlite）与宿主侧（desktop memory /
//! memory_files）共用同一份实现，同一键位必落同一文件。写入边界（宁严勿松）：
//! 全部落点都在数据根 `memory/` 之内。
//!
//! 真源归属按 `docs/adr/0007`（记忆 = 扁平事实文件一处落盘）扳正：v1.0 草案推荐的
//! "sqlite = 机器索引层 + md = 人类层"双层双写**已撤销**，sqlite 侧的读写在 S1 刀② 摘除
//! （本刀只把索引预算钳搬到写侧，sqlite 一行未动 ⇒ 本模块当前仍是"真源已落之后的同步"
//! 那个旧次序，别照这段话理解现状）。
//!
//! 布局（设计 §2.1，与 `scope_dirs` 实况一致）：
//! ```text
//! memory/
//! ├── global/
//! │   ├── MEMORY.md                    # 全局索引（一文件一行，落盘前钳 200 行 / 25_000 字符）
//! │   └── <key>.md                     # 全局记忆（'' 键位：用户偏好、全局要求）
//! └── projects/<可读名-短哈希>/
//!     ├── MEMORY.md                    # 该项目事实索引（坐在项目文件夹根）
//!     ├── tree.md                      # 该项目记忆树的人读层（宿主 memory_files 写）
//!     └── facts/<key>.md               # 项目事实
//! ```
//! 未归属会话的树另有 `_unaffiliated/`（它**不是**项目，见 `UNAFFILIATED_DIR`）。

use std::path::{Path, PathBuf};

use crate::fsutil;

mod sync;

pub use sync::{ParsedFact, atomic_write, mtime_ms, parse_fact, read_marker, write_marker};

/// md 根目录名（数据根下）。
const DIR_NAME: &str = "memory";
/// 项目层目录名。**人类层里 `projects` 只许出现在这一处**——所有项目路径一律
/// 经 `project_rel_dir` / `project_dir` 得出（队列 Q15 立的：新命令
/// `memory_reveal_project` 是这句话的第四个消费者，不收口就要落第五份抄本；
/// 收口前实测 4 处：本文件 2 处 + memory_files.rs 的 tree.md 落点 +
/// memory_reconcile.rs 的 scope_facts_dir）。
const PROJECTS_DIR: &str = "projects";
/// 未归属会话（`''` 键位）在 `projects/` 树里的人读层文件夹名。它**不是**项目：
/// 所以不走 `project_dir_name`（那会造出一个 `<空>-<哈希>` 的伪项目名），但它的
/// 名字与"住在 projects 树下"这件事归本模块——写侧（memory_files 的 tree.md
/// 落点）与读侧（memory_tree_reconcile 的目录反查项目）都引用这里，字面量全仓
/// 只此一处（Q15 探针 P4a：改前散在两个文件）。
pub const UNAFFILIATED_DIR: &str = "_unaffiliated";
/// 索引硬顶：指针行数（设计 §2.1）。**这条预算的全仓唯一出处就在这里**——
/// 写侧（本模块 `refresh_index` 落盘）与注入侧（宿主 `memory_index`）都引用
/// 这两个常量，两份数字一旦各写各的，就会出现"文件里 210 行、注入只给看 200
/// 行"这种人在资源管理器里看到的和模型看到的不一致（v1.1 把钳搬到写侧的目的
/// 正是让人看到的那份 = 模型看到的那份）。
pub const MAX_INDEX_LINES: usize = 200;
/// 索引硬顶：正文字符数（与行数任一先撞线就截，长 key 会先撞字符预算）。
pub const MAX_INDEX_CHARS: usize = 25_000;
/// 截断必须自描述：末行留一条指路，否则人打开索引会以为"本项目就记了这些"。
pub const INDEX_TRUNCATED_NOTE: &str =
    "… index truncated — open this folder to see every file, or use memory_search";
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
    // 取末段时 `\` 与 `/` **同等**当边界。这不是把 Windows 语义搬给 Linux：
    // ① 同一模块里的 `key_file_stem` 一直就是把两种分隔符都当危险字符处理的；
    // ② 这一段只是给人读的名字，唯一性由后面的 fnv 短哈希承担（同名不同路径不冲突），
    //    所以按哪个字符切都不影响寻址；
    // ③ 而在 POSIX 上把整串 `F:\A\B` 当一个名字，会得到一个含反斜杠的目录名——合法，
    //    但白占 40 字符截断额度，还把 `..` 留在名字里（Linux 实测：`a\b\..\c` →
    //    `a_b_.._c`，穿越材料进得了目录名）。
    let readable = workspace
        .rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
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
    // 第二道：连续的点一律压掉。上面那道靠"剥首尾点"挡 `..`，但它管不住 `a..b`
    // 这种夹在中间的形态——目录名里只要还剩 `..`，拼接就有走出 root 的可能。
    let head = head.replace("..", "_");
    let head = head
        .trim_matches(|c: char| c == '.' || c == '_')
        .to_string();
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

/// 项目在人类层的家目录（**相对** md 根）= `projects/<可读名-短哈希>`。
/// 空 workspace 不走这里：它的事实落 `global/`、tree.md 落 `_unaffiliated/`，
/// 两层的特例名本来就不同（Q15 探针 P4a–c 实测），并成一个 = 改磁盘形状。
pub fn project_rel_dir(workspace: &str) -> PathBuf {
    PathBuf::from(PROJECTS_DIR).join(project_dir_name(workspace))
}

/// 项目在人类层的家目录（**绝对**，`root` = `<数据根>/memory`）。队列 Q15：
/// 「打开记忆目录」与后续级联删除都要问这一句，而答案只许有一处算法——
/// `''`（未归属）**拒绝回答**而不是给个目录：未归属不是项目，给它一个目录
/// 名就等于让"删除该项目的记忆数据"这条破坏性通路能吃下全局键位。
pub fn project_dir(root: &Path, workspace: &str) -> Result<PathBuf, String> {
    if workspace.is_empty() {
        return Err(
            "未归属（空 workspace）没有项目目录：事实层在 global/，tree.md 在 _unaffiliated/"
                .to_string(),
        );
    }
    Ok(root.join(project_rel_dir(workspace)))
}

/// `<root>/projects` 本身（**扫描全部项目**的读侧用：记忆 reconcile 与
/// tree.md 对账各自都抄过这一句，Q15 一并收口）。单个项目的家目录用
/// [`project_dir`]，别拿这个再拼一次名字。
pub fn projects_dir(root: &Path) -> PathBuf {
    root.join(PROJECTS_DIR)
}

/// 未归属会话的 tree.md 之家 = `<root>/projects/_unaffiliated`（绝对）。与
/// [`UNAFFILIATED_DIR`] 同处定义，是为了让「projects 这个字面量在人类层只出现
/// 一次」这条判据成立（Q15 立的 A1）。
pub fn unaffiliated_dir(root: &Path) -> PathBuf {
    projects_dir(root).join(UNAFFILIATED_DIR)
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
/// `projects/<dir>/facts/<stem>.md`（项目支一律经 `project_rel_dir`）。
pub fn fact_rel_path(workspace: &str, key: &str) -> PathBuf {
    let file = format!("{}.md", key_file_stem(key));
    if workspace.is_empty() {
        PathBuf::from("global").join(file)
    } else {
        project_rel_dir(workspace).join("facts").join(file)
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
/// （索引坐在项目文件夹根）。项目支经 `project_dir` ⇒ `''` 在这里同样**拒**，
/// 由调用点决定怎么办（`refresh_index` 直接上抛；reconcile 记 warning 跳过）。
fn scope_dirs(root: &Path, workspace: &str) -> Result<(PathBuf, PathBuf), String> {
    if workspace.is_empty() {
        let dir = root.join("global");
        Ok((dir.clone(), dir.join("MEMORY.md")))
    } else {
        let dir = project_dir(root, workspace)?;
        Ok((dir.join("facts"), dir.join("MEMORY.md")))
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

/// 写侧预算钳：行数与字符数**任一先撞线即截**，且只丢整行不写半行。
/// 返回 `(保留的指针行, 是否发生截断)`。头部与每行行尾换行都算进字符预算——
/// 头部不算就会被它悄悄撑爆，行尾不算则 200 行实际比预算多出 200 字符。
fn clamp_index(lines: Vec<String>) -> (Vec<String>, bool) {
    let head = "# 记忆索引\n\n";
    let mut kept: Vec<String> = Vec::new();
    let mut used = head.chars().count();
    for line in lines {
        let cost = line.chars().count() + 1;
        if kept.len() >= MAX_INDEX_LINES || used + cost > MAX_INDEX_CHARS {
            return (kept, true);
        }
        used += cost;
        kept.push(line);
    }
    (kept, false)
}

/// 重建一个键位的 MEMORY.md 索引（一文件一行，文件名稳定序，**落盘前按 §2.1
/// 预算钳**）。facts 目录不存在时也会创建（空索引自描述）。
pub fn refresh_index(root: &Path, workspace: &str) -> Result<(), String> {
    let (dir, index_path) = scope_dirs(root, workspace)?;
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
    let (kept, truncated) = clamp_index(lines);
    let mut body = format!("# 记忆索引\n\n{}", kept.join("\n"));
    if truncated {
        body.push_str(&format!("\n{INDEX_TRUNCATED_NOTE}"));
    }
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("memory 目录创建失败 {}: {e}", dir.display()))?;
    sync::atomic_write(&index_path, &format!("{body}\n"))
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
