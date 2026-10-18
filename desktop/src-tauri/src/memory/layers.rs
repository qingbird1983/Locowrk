//! **每层一行与回收区**（S1 刀④c · ADR-0007 §18「每层一行（文件数 / 更新时间 / 打开
//! 文件夹 / 删除）」+ 刀④a 留下的 `obsolete/` 那三个入口）的宿主侧读数装配。
//!
//! 为什么单独一片（而不是往 `crud.rs` 后面接）：`crud` 管的是**一条**记忆的行形状与读写，
//! 这一片管的是"一层"和"回收区"两个 crud 里没有的维度；并进去就是让一个文件同时管两件事
//! （三问自检第①问答"否"）。命令壳（`commands/memory.rs`）仍然只做参数转换 —— 业务在这里，
//! 界面不写规则（AGENTS §4）。
//!
//! ⚠ 层的身份一律取 `memlayout::layers()` 的那个 `LayerDir`：**不自拼路径**、也不按
//! `StoredFact.workspace` 分组。那是第五处 hint 陷阱（人自己新建的文件那一维只是目录名
//! 兜底，权威原串在真源 = 文件之后已经读不回来），同一条禁令写在 `layer_stats` 的文档里。
//! 「显示哪一层 / 清哪一层 / 开哪个文件夹」三件事都拿 `hint` 回到**同一个** `LayerDir`，
//! 所以它们不可能指到三个不同的地方去。

use std::path::{Path, PathBuf};

use serde::Serialize;

/// 管理页「每层一行」。
#[derive(Debug, Clone, Serialize)]
pub struct LayerRow {
    /// 层身份（`''` = 全局层，其余是 `<可读名>-<哈希>` 目录名）。这一行的主键。
    pub hint: String,
    /// 读得出来的条数。
    pub fact_count: usize,
    /// 该层最新一条的 `metadata.updated`（一条都没有 = 0，前端显示成"—"）。
    pub newest_updated_at: i64,
    /// 该层没能读出来的**文件**数（界面上要如实说出来，不许让上面那个数冒充全量）。
    pub unreadable: usize,
}

impl From<plugin_api::memlayout::LayerStat> for LayerRow {
    fn from(s: plugin_api::memlayout::LayerStat) -> Self {
        Self {
            hint: s.hint,
            fact_count: s.count,
            newest_updated_at: s.newest_at,
            unreadable: s.unreadable,
        }
    }
}

/// 回收区一行（`obsolete/` 里的一份镜像）。
#[derive(Debug, Clone, Serialize)]
pub struct ObsoleteRow {
    /// 相对 `obsolete/` 的镜像路径 = 「恢复」的入参，原样传回即可（`restore_obsolete`
    /// 里 `is_trash_relative` 那道闸认它的形状，越界的当场拒）。
    pub rel: String,
    pub key: String,
    /// 原来在哪一层（同样是 hint）。
    pub workspace: String,
    /// 删除时刻（毫秒，来自文件名上那段 `~<ms>` 删除戳）。
    pub deleted_at_ms: i64,
}

impl From<plugin_api::memlayout::ObsoleteEntry> for ObsoleteRow {
    fn from(e: plugin_api::memlayout::ObsoleteEntry) -> Self {
        Self {
            rel: e.rel.to_string_lossy().into_owned(),
            key: e.key,
            workspace: e.workspace,
            deleted_at_ms: e.deleted_at_ms,
        }
    }
}

/// 层区那一块的完整载荷。**两个汇总数必须在后端算**：界面上"另有 n 层读不到 / 另有 m 篇
/// 读不出"是读数不是样式，前端各写一份 reduce 就是同一句话的两个出处。
#[derive(Debug, Clone, Serialize)]
pub struct LayersView {
    pub rows: Vec<LayerRow>,
    /// 没能扫出来的**层**数。这些层不在 `rows` 里（`layers()` 只枚举读得动的目录），
    /// 所以只能靠这个数交代——把它当 0 处理，"读不到"就成了"没列出来"。
    pub blocked_layers: usize,
    /// 各层 `unreadable` 之和（读得动目录、读不动某一篇）。
    pub unreadable_files: usize,
}

/// 每层一行的读数（读源 = 真源 md，一条 SQL 都不发）。
pub fn layers_view(root: &Path) -> LayersView {
    let (stats, blocked_layers) = plugin_api::memlayout::layer_stats(root);
    let rows: Vec<LayerRow> = stats.into_iter().map(LayerRow::from).collect();
    let unreadable_files = rows.iter().map(|r| r.unreadable).sum();
    LayersView {
        rows,
        blocked_layers,
        unreadable_files,
    }
}

/// 回收区那一块的完整载荷。`unreadable` = 回收区里**没能列出来的条目数**（同一份纪律：
/// 只给一个"看着是空"的 Vec，读不动与从没删过在界面上就成了同一个答案）。
#[derive(Debug, Clone, Serialize)]
pub struct ObsoleteView {
    pub rows: Vec<ObsoleteRow>,
    pub unreadable: usize,
}

/// 回收区清单：按删除时刻**倒序**（用户点开这一块找的是他刚删掉的那一条）。
/// 同一条被反复软删会有多条镜像（落点防撞，见 `stamp_slot`），这里逐条列出不合并。
pub fn obsolete_view(root: &Path) -> ObsoleteView {
    let scan = plugin_api::memlayout::list_obsolete(root);
    let mut rows: Vec<ObsoleteRow> = scan.entries.into_iter().map(ObsoleteRow::from).collect();
    rows.sort_by_key(|r| std::cmp::Reverse(r.deleted_at_ms));
    ObsoleteView {
        rows,
        unreadable: scan.unreadable,
    }
}

/// 「删除该层」= 把那一层的事实文件全部搬进回收区（可恢复；真删只在"清空回收区"那一次）。
pub fn clear_layer(root: &Path, hint: &str) -> Result<usize, String> {
    plugin_api::memlayout::soft_clear_layer(root, hint)
}

/// 「恢复」一条。`Ok(false)` = 回收区里没这条了（列表过期，前端据此刷新）。
pub fn restore_obsolete(root: &Path, rel: &str) -> Result<bool, String> {
    plugin_api::memlayout::restore_obsolete(root, Path::new(rel))
}

/// 「清空回收区」= 唯一真删那一次（界面上必须带二次确认）。返回删掉的条数。
pub fn purge_obsolete(root: &Path) -> Result<usize, String> {
    plugin_api::memlayout::purge_obsolete(root)
}

/// 「打开该层文件夹」要开的目录。全局层开 `global/` 本身；项目层开它的**项目文件夹**
/// （`facts/` 的上一级——`MEMORY.md` 与 `tree.md` 都坐在那儿），与
/// `memory_reveal_project` 同一形状，两个按钮不该指出两个地方。
///
/// 找不到那一层 ⇒ `Err` 而不是回落别处：静默开一个相邻目录比报错难查得多。
pub fn layer_dir_to_open(root: &Path, hint: &str) -> Result<PathBuf, String> {
    let (dirs, _blocked) = plugin_api::memlayout::layers(root);
    let layer = dirs
        .iter()
        .find(|l| l.hint == hint)
        .ok_or_else(|| format!("记忆里没有这一层（{hint}）"))?;
    if hint.is_empty() {
        return Ok(layer.facts.clone());
    }
    layer
        .facts
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or_else(|| format!("记忆层目录算不出上一级：{}", layer.facts.display()))
}
