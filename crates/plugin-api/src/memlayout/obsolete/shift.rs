//! **回收区的写面**（S1 刀④a 立、刀④c 补齐）：软删一条 / 软删一层 / 软删全部 /
//! 恢复一条 / 真删整个回收区。"回收区里每条是谁、从哪来"那一半（身份与镜像路径的解释）
//! 住在父模块，这一片只管**搬**。
//!
//! 为什么拆（刀④c 把单文件顶到 427 行）：三问自检第①问答"否"——一个文件同时管读面与
//! 写面，而这次涨的偏偏全是写面（单层清空 + 恢复的越界拒）。两片各自只管一件事之后，
//! 改落点算法不会牵动清单字段，反之亦然。
//!
//! ⚠ 三条布局纪律（镜像原 rel、读面天然看不见 `obsolete/`、软删不动 `.sync` 标记）
//! 写在父模块头注里：那三句是**两边**的约束，不在这里重抄一份（两处抄写必漂）。

use std::path::{Path, PathBuf};

use crate::fsutil;

use crate::memlayout::{
    LayerDir, fact_rel_path, index::refresh_layer, layers, read::is_fact_file, sync,
};

/// 给回收区里这个 stem 选一个**没被占用**的删除戳：从 `ms` 起，撞上就 +1ms 重试，
/// 上限 100 次（撞满 ⇒ `None`，宁可报错也不静默少一条）。
///
/// 为什么抽成收 `taken` 的纯函数：防撞这件事必须能被**确定性地**测。生产那层传
/// `Path::exists()`，而测试里"同一毫秒连删两次"是运气（两次墙钟读数跨不跨毫秒，
/// 第一次跑就发现它给不出稳定红证）。把判定收进函数之后，"占住前 100 个槽"这种形状
/// 才谈得上被机器证明（红证 = `obsolete.rs` 那两条纯函数用例 + CASE 42 的变异）。
/// 挪的是**戳**不是名字：[`super::strip_stamp`] 只认 `~<全数字>`，加 `-2` 那种后缀会把戳读成
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
        base.join(super::rel_with_last(rel, format!("{stem}~{t}.md")))
            .exists()
    })
    .ok_or_else(|| format!("memory 软删在 {} 撞名超过 100 次", base.display()))?;
    Ok(base.join(super::rel_with_last(rel, format!("{stem}~{stamp}.md"))))
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
    let base = super::obsolete_root(root);
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

/// **全部层软删**（管理页"清空"走这条）：把每一层的事实文件全部搬进 `obsolete/`。
///
/// 返回搬动条数。有一层扫不动 ⇒ 中止并 `Err`，一条也不动——与 `clear_facts` 同一条律：
/// "说清了其实没清"比失败更难被发现。删除戳逐条 +1 递增：同一毫秒里搬多篇是常态，
/// 撞名会让一篇凭空消失。
pub fn soft_clear_facts(root: &Path) -> Result<usize, String> {
    let (dirs, blocked) = layers(root);
    check_blocked(blocked)?;
    let base_ms = sync::now_ms();
    let mut n = 0usize;
    for layer in &dirs {
        move_layer_facts(root, layer, base_ms, &mut n)?;
    }
    // 搬完再统一刷各层索引：中途失败时索引仍与磁盘一致（不会出现"文件在、索引没了"）
    for layer in &dirs {
        if layer.facts.is_dir() {
            refresh_layer(&layer.facts, &layer.index)?;
        }
    }
    Ok(n)
}

/// **单层软删**（管理页"删除该层"，S1 刀④c）：只搬那一层的事实文件，其余层一个字都不碰。
///
/// `hint` 是 [`crate::memlayout::LayerDir::hint`] 那个**给人看的层名**（`''` = 全局层，其余是
/// `<可读名>-<哈希>` 目录名）。⚠ 这里**没有**拿它寻址：它在 [`layers`] 的结果里选中那一层，
/// 路径仍取自同一个 `LayerDir`。"拿 hint 去拼 `fact_rel_path`"正是刀②c-1 实测踩到的第五处
/// hint 陷阱，那条禁令一个字没松——界面上"这一层"的身份从此只能是目录名，权威原串在真源
/// = 文件之后已经读不回来了。
///
/// 找不到那一层 ⇒ `Ok(0)` 而不是 `Err`：层还没被建出来与里面本来就是空的，对用户是同一件
/// 事（"删掉一个不存在的层"不该弹一个错误给他看）。
pub fn soft_clear_layer(root: &Path, hint: &str) -> Result<usize, String> {
    let (dirs, blocked) = layers(root);
    check_blocked(blocked)?;
    let Some(layer) = dirs.iter().find(|l| l.hint == hint) else {
        return Ok(0);
    };
    let mut n = 0usize;
    move_layer_facts(root, layer, sync::now_ms(), &mut n)?;
    if layer.facts.is_dir() {
        refresh_layer(&layer.facts, &layer.index)?;
    }
    Ok(n)
}

/// 扫不动的层数非零 ⇒ 中止。这句话在两个入口上是同一个判据，所以只许有一个出处：
/// 两份文案迟早会漂成"一处说中止一处说跳过"，而那种差别在磁盘上是看不出来的。
fn check_blocked(blocked: usize) -> Result<(), String> {
    if blocked == 0 {
        return Ok(());
    }
    Err(format!(
        "memory 真源有 {blocked} 个层目录扫不动（软删清空中止，未改动任何文件）"
    ))
}

/// 把**一层**里的事实文件全部搬进回收区（`soft_clear_facts` 与 [`soft_clear_layer`] 共用
/// 这一半）：搬移姿势一旦有两份，"清空全部"与"删单一层"就会漂成两种磁盘行为，而那正是
/// 界面上最难看出来的差别。`n` 是跨层连续的序号 ⇒ 同一毫秒里多篇不撞名（见 [`stamp_slot`]
/// 那 100 次重试的唯一红证）。
fn move_layer_facts(
    root: &Path,
    layer: &LayerDir,
    base_ms: i64,
    n: &mut usize,
) -> Result<(), String> {
    let rd = match std::fs::read_dir(&layer.facts) {
        Ok(rd) => rd,
        Err(e) if fsutil::is_not_found(&e) => return Ok(()),
        Err(e) => {
            return Err(format!(
                "memory 软删清空时读不动 {}: {e}",
                layer.facts.display()
            ));
        }
    };
    for p in rd.flatten().map(|f| f.path()).filter(|p| is_fact_file(p)) {
        let rel = p.strip_prefix(root).unwrap_or(&p).to_path_buf();
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
        let target = stamp_slot(&super::obsolete_root(root), &rel, stem, base_ms + *n as i64)?;
        if let Some(dir) = target.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("memory 回收目录创建失败 {}: {e}", dir.display()))?;
        }
        std::fs::rename(&p, &target)
            .map_err(|e| format!("memory 软删清空搬不动 {}: {e}", p.display()))?;
        *n += 1;
    }
    Ok(())
}

/// 回收区里那条镜像路径的**合法形状**：相对、非空、每一段都是普通名字。
/// `RootDir` / `ParentDir` / `CurDir` 任意一种出现都拒——它们不是"回收区里的某一条"，
/// 而是"前端想指到哪儿"。空 rel 也拒：`super::obsolete_root(join(""))` 指到回收区根目录本身，
/// 那更不是一个文件。
fn is_trash_relative(rel: &Path) -> bool {
    !rel.as_os_str().is_empty()
        && rel
            .components()
            .all(|c| matches!(c, std::path::Component::Normal(_)))
}

/// **恢复**一条：按镜像 rel 原路搬回（剥删除戳）。`Ok(false)` = 回收区里没有这条。
///
/// 原位置已被占用（删掉之后又写回过同一个 key）⇒ `Err` 且回收条目**原地不动**：静默覆盖
/// 会把用户新写的那一条吃掉，那比"恢复失败、让他看一眼"严重得多。
///
/// ⚠ `rel` 必须是**纯相对、逐段 Normal**（就是 [`list_obsolete`] 列出来的那种形状）。
/// 界面上"恢复"是刀④c 新加的一枚按钮，它的入参从此由"我们列出来的东西"变成"前端递给
/// 什么"——不拦 `..`，一条 `../../x.md` 就能把根外一个文件搬进真源，那是把一个删除面
/// 写成任意路径读写。这道校验放在**这里**而不是命令壳里：薄壳拦不住下一个调用者。
pub fn restore_obsolete(root: &Path, rel: &Path) -> Result<bool, String> {
    if !is_trash_relative(rel) {
        return Err(format!(
            "回收区路径不合法（只认 obsolete/ 下镜像原位的相对路径）: {}",
            rel.display()
        ));
    }
    let from = super::obsolete_root(root).join(rel);
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
    let (head, _) = super::strip_stamp(stem);
    let origin = super::rel_with_last(rel, format!("{head}.md"));
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
    let base = super::obsolete_root(root);
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
