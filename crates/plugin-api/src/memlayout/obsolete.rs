//! **软删面**（S1 刀④a · AC-S1-7）：删除一条记忆 = 把它搬进 `<根>/obsolete/`；恢复 =
//! 原路搬回；清空 `obsolete/` 才是真删，而那一下是用户自己的手势。
//!
//! 为什么要有这一层：真源就是文件，"删"从此是一个**磁盘动作**。旧形状里删除还有 undo
//! 载荷兜着（库里那一行 + 工具返回的 `previous_entry`），而管理页删掉一条之后那个载荷
//! 早就不在了——用户后悔时唯一能救他的是文件历史记录。`obsolete/` 把"删"变成可逆的，
//! 不可逆只发生在他点"清空回收"并确认的那一次。
//!
//! 三条布局纪律：
//! ① `obsolete/` **镜像原 rel**（`global/x.md`、`projects/<可读名-哈希>/facts/x.md`），
//!    文件名主干追加 `~<删除时毫秒>`。同一条被反复软删不撞车，且人打开这个目录就看得见
//!    "它原来住在哪一层、什么时候删的"。恢复按镜像路径搬回，**不拿 frontmatter 的
//!    `workspace` 寻址**——那是本项目第五处 hint 陷阱：人自己建的文件那一维可能整个缺失，
//!    缺了就恢复不回它来的地方。层定位一律经 [`super::layers`]（路径算法不许有第二处）。
//! ② 读面（[`super::layers`] / `iter_facts` / `facts_of` / `read_fact` / `clear_facts`）
//!    天然看不见 `obsolete/`：它们只走 `global/` 与 `projects/*/facts/`，本目录是它们的
//!    兄弟而不是子目录。这条"删除之后注入零命中"不靠约定——硬删 API（`delete_fact` /
//!    `clear_facts`）在全生产调用面上被收成**一个白名单点**：`memory_undo` 撤销一次新建
//!    （当作没发生过，不该留在回收区）。闸 = `check-single-source.mjs` 的 `SS15a/SS15b`，
//!    口径是**文件集合**，所以"新增一处硬删"与"白名单那处搬走了"都当场红。
//! ③ 软删**不动 `.sync/` 标记**。保留它，reconcile 那一轮看见的是"标记在 + 行在 + 文件缺"
//!    ⇒ 走删行支；把标记一起删掉，影子行就成了没有主人的幽灵，界面上表现为
//!    "删了又冒出来"。

use std::path::{Path, PathBuf};

use crate::fsutil;

use super::{fact_rel_path, index::refresh_layer, layers, read::is_fact_file, sync};

/// 回收目录名（`<根>/obsolete`）。
pub const OBSOLETE_DIR: &str = "obsolete";

/// 回收根目录（不做存在性判断——它不存在时列出来就是空）。
pub fn obsolete_root(root: &Path) -> PathBuf {
    root.join(OBSOLETE_DIR)
}

/// 剥掉 `~<毫秒>` 删除戳，得到原文件名主干。没有那段 = 主干原样、时刻 0（人手动放进来的）。
fn strip_stamp(stem: &str) -> (&str, i64) {
    match stem.rsplit_once('~') {
        Some((head, tail)) if !head.is_empty() && tail.chars().all(|c| c.is_ascii_digit()) => {
            (head, tail.parse().unwrap_or(0))
        }
        _ => (stem, 0),
    }
}

/// 把 rel 的最后一段换成 `new_name`，其余分段原样保留（回收区内外的路径形状互换都靠它）。
fn rel_with_last(rel: &Path, new_name: String) -> PathBuf {
    let mut out = PathBuf::new();
    let comps: Vec<_> = rel.components().collect();
    for (i, c) in comps.iter().enumerate() {
        if i + 1 == comps.len() {
            out.push(new_name.as_str());
        } else {
            out.push(c.as_os_str());
        }
    }
    out
}

/// 一条躺在 `obsolete/` 里的记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObsoleteEntry {
    /// `obsolete/` 之下的相对路径（**恢复的定位符**，含删除戳）
    pub rel: PathBuf,
    /// 原 rel（镜像目录 + 剥戳主干）——恢复时搬回去的位置
    pub origin: PathBuf,
    /// 删除时刻（毫秒；文件名里那段，缺 = 0）
    pub deleted_at_ms: i64,
    /// 这条记忆的 key：frontmatter `name` 优先，缺 = 剥戳后的文件名主干
    pub key: String,
    /// 只做展示的一维：frontmatter `workspace`（缺 = 空串）。
    /// ⚠ **不许拿它寻址**（纪律①那处 hint 陷阱）；寻址一律走 [`Self::origin`]。
    pub workspace: String,
}

/// 回收区点名结果。
#[derive(Debug, Default)]
pub struct ObsoleteScan {
    pub entries: Vec<ObsoleteEntry>,
    /// 读不动的条目数（坏 frontmatter / 某层扫不动）：条目照样列出、字段走兜底，
    /// 但"没给看的条数"必须可观测——它与"回收区是空的"是两个意思。
    pub unreadable: usize,
}

/// 点名 `obsolete/`（递归，只认事实文件：`.md` 且不是索引/投影）。按删除时刻**降序**。
///
/// 目录本身不在 = 空回收区（不是故障）；`obsolete/` 这个位置被普通文件冒充 ⇒ 整区扫不动，
/// 记进 `unreadable`（不是"空"）。
pub fn list_obsolete(root: &Path) -> ObsoleteScan {
    let base = obsolete_root(root);
    let mut out = ObsoleteScan::default();
    match std::fs::metadata(&base) {
        Err(_) => return out,
        Ok(md) if !md.is_dir() => {
            out.unreadable = 1;
            return out;
        }
        Ok(_) => {}
    }
    let mut stack: Vec<PathBuf> = vec![base.clone()];
    while let Some(dir) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => {
                out.unreadable += 1;
                continue;
            }
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
                continue;
            }
            if !is_fact_file(&p) {
                continue;
            }
            let rel = p.strip_prefix(&base).unwrap_or(&p).to_path_buf();
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let (head, deleted_at_ms) = strip_stamp(stem);
            let mut entry = ObsoleteEntry {
                rel: rel.clone(),
                origin: rel_with_last(&rel, format!("{head}.md")),
                deleted_at_ms,
                key: head.to_string(),
                workspace: String::new(),
            };
            match std::fs::read_to_string(&p) {
                Ok(text) => {
                    let parsed = sync::parse_fact(&text);
                    if let Some(name) = parsed.name.filter(|s| !s.is_empty()) {
                        entry.key = name;
                    }
                    entry.workspace = parsed.workspace.unwrap_or_default();
                }
                Err(_) => out.unreadable += 1,
            }
            out.entries.push(entry);
        }
    }
    out.entries.sort_by(|a, b| {
        b.deleted_at_ms
            .cmp(&a.deleted_at_ms)
            .then_with(|| a.rel.cmp(&b.rel))
    });
    out
}

/// 给回收区里这个 stem 选一个**没被占用**的删除戳：从 `ms` 起，撞上就 +1ms 重试，
/// 上限 100 次（撞满 ⇒ `None`，宁可报错也不静默少一条）。
///
/// 为什么抽成收 `taken` 的纯函数：防撞这件事必须能被**确定性地**测。生产那层传
/// `Path::exists()`，而测试里"同一毫秒连删两次"是运气（两次墙钟读数跨不跨毫秒，
/// 第一次跑就发现它给不出稳定红证）。把判定收进函数之后，"占住前 100 个槽"这种形状
/// 才谈得上被机器证明（红证 = `obsolete.rs` 那两条纯函数用例 + CASE 42 的变异）。
/// 挪的是**戳**不是名字：[`strip_stamp`] 只认 `~<全数字>`，加 `-2` 那种后缀会把戳读成
/// 主干的一部分，恢复时就把文件放回一个叫 `x~1728-2` 的 key 上。
pub(crate) fn slot_stamp(ms: i64, taken: impl Fn(i64) -> bool) -> Option<i64> {
    for extra in 0..=100i64 {
        if !taken(ms + extra) {
            return Some(ms + extra);
        }
    }
    None
}

/// 在回收区里给这个 rel 找一个不撞名的落点（[`slot_stamp`] + 拼路径）。
fn stamp_slot(base: &Path, rel: &Path, stem: &str, ms: i64) -> Result<PathBuf, String> {
    let stamp = slot_stamp(ms, |t| {
        base.join(rel_with_last(rel, format!("{stem}~{t}.md")))
            .exists()
    })
    .ok_or_else(|| format!("memory 软删在 {} 撞名超过 100 次", base.display()))?;
    Ok(base.join(rel_with_last(rel, format!("{stem}~{stamp}.md"))))
}

/// **软删一条**：搬进 `obsolete/`（镜像原 rel + 删除戳），并刷新原层的 `MEMORY.md`。
///
/// `Ok(false)` = 该键位下这条文件本来就不在（幂等 no-op）。`Err` = 层级故障（那一层的
/// 位置被普通文件冒充）或搬移/刷新失败——**与"没有这条"分开报**，否则"整层读不动"会被
/// 界面读成"这条不存在"。纪律③：这里不删 `.sync/` 标记。
pub fn soft_delete_fact(root: &Path, workspace: &str, key: &str) -> Result<bool, String> {
    let rel = fact_rel_path(workspace, key);
    let from = root.join(&rel);
    let stem = from
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let base = obsolete_root(root);
    let target = stamp_slot(&base, &rel, stem, sync::now_ms())?;
    if let Some(dir) = target.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("memory 回收目录创建失败 {}: {e}", dir.display()))?;
    }
    match std::fs::rename(&from, &target) {
        Ok(()) => {}
        Err(e) if fsutil::is_not_found(&e) => {
            if let Some(parent) = from.parent() {
                if parent.exists() && !parent.is_dir() {
                    return Err(format!(
                        "memory 层不是目录（软删无法判定） {}: {e}",
                        parent.display()
                    ));
                }
            }
            return Ok(false);
        }
        Err(e) => return Err(format!("memory 软删失败 {}: {e}", from.display())),
    }
    refresh_layer_of(root, &rel)?;
    Ok(true)
}

/// 刷新 `rel`（相对数据根）那个文件**所在层**的索引。层表取自 [`layers`]，本片不自拼
/// `projects/<x>/facts`——目录形状只许有一处算法（Q15 那条口径）。找不到 = 不动，
/// 交给下一轮 reconcile 收敛（索引不是真源，漂了会被对账扳正）。
fn refresh_layer_of(root: &Path, rel: &Path) -> Result<(), String> {
    let facts = match rel.parent() {
        Some(p) => root.join(p),
        None => return Ok(()),
    };
    let (dirs, _blocked) = layers(root);
    for l in dirs {
        if l.facts == facts {
            return refresh_layer(&l.facts, &l.index);
        }
    }
    Ok(())
}

/// **整层软删**（管理页"清空"走这条）：把每一层的事实文件全部搬进 `obsolete/`。
///
/// 返回搬动条数。有一层扫不动 ⇒ 中止并 `Err`，一条也不动——与 `clear_facts` 同一条律：
/// "说清了其实没清"比失败更难被发现。删除戳逐条 +1 递增：同一毫秒里搬多篇是常态，
/// 撞名会让一篇凭空消失。
pub fn soft_clear_facts(root: &Path) -> Result<usize, String> {
    let (dirs, blocked) = layers(root);
    if blocked > 0 {
        return Err(format!(
            "memory 真源有 {blocked} 个层目录扫不动（软删清空中止，未改动任何文件）"
        ));
    }
    let base_ms = sync::now_ms();
    let mut n = 0usize;
    for layer in &dirs {
        let rd = match std::fs::read_dir(&layer.facts) {
            Ok(rd) => rd,
            Err(e) if fsutil::is_not_found(&e) => continue,
            Err(e) => {
                return Err(format!(
                    "memory 软删清空时读不动 {}: {e}",
                    layer.facts.display()
                ));
            }
        };
        let items: Vec<PathBuf> = rd
            .flatten()
            .map(|f| f.path())
            .filter(|p| is_fact_file(p))
            .collect();
        for p in items {
            let rel = p.strip_prefix(root).unwrap_or(&p).to_path_buf();
            let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let target = stamp_slot(&obsolete_root(root), &rel, stem, base_ms + n as i64)?;
            if let Some(dir) = target.parent() {
                std::fs::create_dir_all(dir)
                    .map_err(|e| format!("memory 回收目录创建失败 {}: {e}", dir.display()))?;
            }
            std::fs::rename(&p, &target)
                .map_err(|e| format!("memory 软删清空搬不动 {}: {e}", p.display()))?;
            n += 1;
        }
    }
    // 搬完再统一刷各层索引：中途失败时索引仍与磁盘一致（不会出现"文件在、索引没了"）
    for layer in &dirs {
        if layer.facts.is_dir() {
            refresh_layer(&layer.facts, &layer.index)?;
        }
    }
    Ok(n)
}

/// **恢复**一条：按镜像 rel 原路搬回（剥删除戳）。`Ok(false)` = 回收区里没有这条。
///
/// 原位置已被占用（删掉之后又写回过同一个 key）⇒ `Err` 且回收条目**原地不动**：静默覆盖
/// 会把用户新写的那一条吃掉，那比"恢复失败、让他看一眼"严重得多。
pub fn restore_obsolete(root: &Path, rel: &Path) -> Result<bool, String> {
    let from = obsolete_root(root).join(rel);
    match std::fs::symlink_metadata(&from) {
        Ok(md) if md.is_file() => {}
        Ok(_) => return Err(format!("回收区里那个不是文件 {}", from.display())),
        Err(e) if fsutil::is_not_found(&e) => return Ok(false),
        Err(e) => return Err(format!("memory 恢复读不到 {}: {e}", from.display())),
    }
    let stem = from
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let (head, _) = strip_stamp(stem);
    let origin = rel_with_last(rel, format!("{head}.md"));
    let to = root.join(&origin);
    if to.exists() {
        return Err(format!(
            "原位置已有同名文件 {}（先处理那一条再恢复）",
            to.display()
        ));
    }
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("memory 恢复目录创建失败 {}: {e}", dir.display()))?;
    }
    std::fs::rename(&from, &to)
        .map_err(|e| format!("memory 恢复搬不动 {}: {e}", from.display()))?;
    // 恢复回来的文件带着它自己的 mtime ⇒ 下一轮 reconcile 看见"文件在 + 行缺"走新增支，
    // 把影子行按文件内容建回来（内容以文件为准）。
    refresh_layer_of(root, &origin)?;
    Ok(true)
}

/// **真删**：清空 `obsolete/`——回收区里的东西只有这一处能删掉，而它是界面上"清空回收区"
/// 那一个带二次确认的手势（`SS15a/SS15b` 把硬删收成"撤销新建"一个白名单点，就是为了让
/// 这句话可以被机器判）。
///
/// 返回删除条数。列目录读不动 ⇒ 中止并 `Err`（一条也不删）。搬空的子目录一并收掉，
/// 但 `obsolete/` 本身留着——它是布局的一部分，不该"清空"之后消失。
pub fn purge_obsolete(root: &Path) -> Result<usize, String> {
    let base = obsolete_root(root);
    match std::fs::metadata(&base) {
        Err(_) => return Ok(0),
        Ok(md) if !md.is_dir() => {
            return Err(format!("memory 回收区不是目录 {}", base.display()));
        }
        Ok(_) => {}
    }
    let mut doomed: Vec<PathBuf> = Vec::new();
    let mut subdirs: Vec<PathBuf> = Vec::new();
    let mut stack = vec![base.clone()];
    while let Some(dir) = stack.pop() {
        let rd = std::fs::read_dir(&dir)
            .map_err(|e| format!("memory 回收区读不动 {}: {e}", dir.display()))?;
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                subdirs.push(p.clone());
                stack.push(p);
            } else if is_fact_file(&p) {
                doomed.push(p);
            }
        }
    }
    let mut n = 0usize;
    for p in doomed {
        std::fs::remove_file(&p)
            .map_err(|e| format!("memory 清空回收区删不掉 {}: {e}", p.display()))?;
        n += 1;
    }
    subdirs.sort_by(|a, b| b.cmp(a));
    for d in subdirs {
        let _ = std::fs::remove_dir(&d); // 非空即失败：忽略，不报错
    }
    Ok(n)
}
