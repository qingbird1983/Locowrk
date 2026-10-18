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
//!
//! 这一片只管**身份与镜像**（回收区里每条是谁、原来住在哪一层、什么时候删的、扫不扫得动）；
//! 四个磁盘动作（软删一条 / 软删一层 / 恢复 / 真删）在子片 [`shift`]——S1 刀④c 把它拆出去，
//! 因为那三问自检第①问答"否"：单文件同时管读面与写面，而这轮涨的偏偏全是写面。上面那三条
//! 布局纪律是**两边**的约束，所以只写在这里一次。

use std::path::{Path, PathBuf};

use super::{read::is_fact_file, sync};

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

/// 写面（软删 / 清空 / 恢复 / 真删）单独一片。`pub(crate)` 只为让 `slot_stamp` 那颗纯函数
/// 能被测试直接喂"前 100 个槽都被占"这种确定性形状——生产那层传的是 `Path::exists()`，
/// 拿它测撞名要靠两次墙钟落在同一毫秒里，那种红证等于零（CASE 42 打空过一次就是这么来的）。
pub(crate) mod shift;

pub use shift::{
    purge_obsolete, restore_obsolete, soft_clear_facts, soft_clear_layer, soft_delete_fact,
};
