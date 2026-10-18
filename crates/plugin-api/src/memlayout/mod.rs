//! 记忆文件化布局（F1，《Memory File-ization Design v1.1》）：记忆的唯一真源 =
//! `<数据根>/memory/` 下的 Markdown——人可查可读可写，编辑即权威写入。
//! 本模块只放**渲染与写删**（frontmatter 怎么落、一条事实写到哪、删完刷谁）——
//! 寻址面（目录命名、文件名安全、层枚举）在 `paths.rs`，索引面在 `index.rs`，
//! 读回与清点名在 `read.rs`，同步机械件（标记 / 原子写 / 解析 / 冲突备份）在 `sync.rs`，
//! 四类型与落点层在 `kind.rs`。
//! 五片都经这里 `pub use` 出去 ⇒ 插件侧（memory-sqlite）与宿主侧（desktop memory /
//! `memory_files`）吃的仍是同一个 `memlayout::…` 路径，同一键位必落同一文件。
//! 写入边界（宁严勿松）：全部落点都在数据根 `memory/` 之内。
//!
//! 真源归属按 `docs/adr/0007`（记忆 = 扁平事实文件一处落盘）扳正：v1.0 草案推荐的
//! "sqlite = 机器索引层 + md = 人类层"双层双写**已撤销**。⚠ 但**别照这句话读现状**——
//! 撤销是逐面完成的，两侧进度不同：宿主读面（管理页列举 / `<persistent_memory>` 正文 /
//! 项目索引）自 S1 刀②b 起只读这里，宿主写面自刀②c-1 起**先落这里、失败就是失败**；
//! `plugins-src/memory-sqlite` 那六枚工具仍按库寻址，那是刀③b 的"工具内部改道"。
//! 四类型（`metadata.type`）自刀③a 起有了生产者（工具参数 `type`），值域、落点层与
//! `Why:` / `How to apply:` 提醒三件都在 `kind.rs`。
//! 谁在反着写（库 → 文件）也记在 `write_fact` 上方与 specs/05，不靠本段记忆。
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

mod index;
mod kind;
mod paths;
mod read;
mod sync;

// 索引面与读面各自成片，对外路径仍走这里 ⇒ 消费点（宿主 memory_files /
// memory_reconcile / memory_index、插件 store）一个字都不用动。
pub use index::{INDEX_TRUNCATED_NOTE, MAX_INDEX_CHARS, MAX_INDEX_LINES, refresh_index};
pub use kind::{
    Layer, MEMORY_TYPES, UNCLASSIFIED_TYPE, layer_for, missing_guidance, normalize_type,
};
pub use paths::{
    LayerDir, UNAFFILIATED_DIR, fact_rel_path, files_enabled, fnv1a64, key_file_stem, layers,
    memory_root, project_dir, project_dir_name, project_rel_dir, projects_dir, unaffiliated_dir,
};
pub use read::{FactsScan, StoredFact, clear_facts, fact_matches, facts_of, iter_facts, read_fact};
pub use sync::{ParsedFact, atomic_write, mtime_ms, parse_fact, read_marker, write_marker};

/// 给 `index.rs`（写索引）与 `read.rs`（读一层）共用的按键位算路径——不对外，
/// 因为它与 `paths.rs` 里的 `GLOBAL_DIR` 必须一起走（那条字面量在本模块只许有
/// 定义处一处）。测试点名的另两件（`GLOBAL_DIR` / `enabled_from`）**不在这里再导出**：
/// 生产代码不引用它们，挂在 `mod.rs` 会是一条 unused import，所以由
/// `tests/mod.rs` 自己去 `paths` 取（见那边的注释）。
pub(crate) use paths::scope_dirs;

/// 一条事实（双写与导出的渲染输入）。
pub struct FactEntry<'a> {
    pub workspace: &'a str,
    pub key: &'a str,
    pub value: &'a str,
    pub tags: &'a str,
    /// explicit | extracted（zcode 式 frontmatter 之外 Locowrk 自有溯源）。
    pub source: &'a str,
    pub updated_at: i64,
    /// `metadata.type` 的输入（设计 §2.1 四类型）。**追加在末尾** = `plugin-api`
    /// 的规矩（只往结构体尾部加字段，既有签名不动）。空串与值域外的值由
    /// [`normalize_type`] 收成 [`UNCLASSIFIED_TYPE`] ⇒ 四类型落地之前落的每个文件
    /// 形状一字不变（担保 = `memlayout/tests/kind.rs::an_entry_without_a_type_renders_the_old_bytes`）。
    pub ty: &'a str,
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

    /// 单文件全文：frontmatter（name / description / tags / metadata）+ 正文。
    /// workspace 与 name 落盘 = F2 reconcile 的身份读回依据。
    ///
    /// `tags` 单独一行是**读侧迁移的前置**：`description` 是"tags，空则正文首行"的
    /// 有损 hook，从它反推不出"这串是标签还是正文第一句"。真源 = 文件之后，管理页
    /// 过滤与 `memory_search` 的 tags 匹配只能从这行取（设计 §2.1）。
    ///
    /// `metadata.type` 走 [`normalize_type`]：调用方给什么串（含空）都落进值域，
    /// 未给 = `fact`（未分类）⇒ 与四类型落地前的字节形完全一致。
    pub fn render(&self) -> String {
        format!(
            "---\nname: {}\ndescription: {}\ntags: {}\nmetadata:\n  type: {}\n  source: {}\n  workspace: {}\n  updated: {}\n---\n\n{}\n",
            yaml_str(self.key),
            yaml_str(&self.description()),
            yaml_str(self.tags),
            yaml_str(&normalize_type(self.ty)),
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

/// 写一条事实 md（目录惰性建 + **冲突备份** + 刷新该键位索引）。幂等覆盖。
/// 返回值 = 文件路径（日志/测试用）。
///
/// ⚠ 这句话在 S1 刀②c-1 之前是「调用方保证 sqlite 已是真源」——**次序已经反过来**：
/// 宿主四条写路径先调这里，这里成功才去写 sqlite 影子（`desktop/src-tauri/src/memory/crud.rs`
/// 的 `write_md` + `shadow_*`）。插件那侧仍按旧次序，直到刀③ 改道。
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
///
/// ⚠ "不存在"与"所在那一层根本不是目录"在 **Windows 上是同一个错误码**
/// （`ERROR_PATH_NOT_FOUND`，路径里任何一段不是目录就报它），POSIX 上才是两个
/// （`ENOENT` / `ENOTDIR`）。不分开就会落一条"Linux 报错、Windows 谎称没这条记忆"的
/// 跨平台假性质——而"没这条"与"这一层读不动"必须是两个答案（S1 刀②c-1 的整条纪律：
/// 真源读不动的时候不许冒充真源是空的）。红证 = 宿主
/// `memory/tests/legacy.rs::a_failed_md_write_is_an_error_and_writes_no_shadow`
/// （夹具两侧同形：在层目录的位置上放一个普通文件）。
pub fn delete_fact(root: &Path, workspace: &str, key: &str) -> Result<bool, String> {
    let rel = fact_rel_path(workspace, key);
    let path = root.join(&rel);
    match std::fs::remove_file(&path) {
        Ok(()) => {}
        Err(e) if fsutil::is_not_found(&e) => {
            if let Some(parent) = path.parent() {
                if parent.exists() && !parent.is_dir() {
                    return Err(format!(
                        "memory 层不是目录（删除无法判定） {}: {e}",
                        parent.display()
                    ));
                }
            }
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
