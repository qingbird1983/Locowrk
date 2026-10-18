//! 事实文件的**读回面**（S1 刀②a：真源 = 文件之后，"读"只能从这里来）。
//!
//! 单独一片的理由：它管的是"按键位取一条"和"扫全根"两种查法，跟渲染、写路径、
//! 索引各是不同件事。硬约束一条：**frontmatter 只有 `sync::parse_fact` 一种读法**，
//! 这片不许自己再 split 一遍——两处解析同一格式迟早读出不一致的字段。

use std::path::{Path, PathBuf};

use crate::fsutil;

use super::{GLOBAL_DIR, fact_rel_path, projects_dir, sync};
use sync::ParsedFact;

/// 一条已落盘事实的读回形状（字段 = frontmatter 各行的并集，缺失按渲染侧同一兜底）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFact {
    /// 键位身份：frontmatter `name`，无则回落文件名主干（设计 §3——文件名只是
    /// 键位的净空编码，人是可以先建文件再让机器吸纳的）。
    pub key: String,
    /// 所在层。frontmatter `workspace` 优先；**缺值时是目录反查的兜底**——全局层
    /// 给 `""`，项目层给那个 `<可读名>-<哈希>` 目录名（它不是权威原串，只给人看）。
    pub workspace: String,
    /// 正文（渲染侧写什么就是什么；无 frontmatter 的文件 = 全文）。
    pub value: String,
    /// frontmatter `tags`。这行是一等字段之前落的旧文件 = 空串，不回填、不重写。
    pub tags: String,
    /// metadata.source（缺 = `"explicit"`，与渲染侧同一兜底）。
    pub source: String,
    /// metadata.updated（缺 / 坏 = 0）。
    pub updated_at: i64,
    pub path: PathBuf,
}

/// 一个键位目录的扫描结果。`unreadable` 是**读得到目录、读不动文件**的条数：
/// 召回侧那条「库不可用不许冒充库是空的」的纪律（宿主 `memory/recall.rs`）搬到
/// 文件制之后就靠这个字段活着——空 `facts` 与"扫不动"必须能分开。
#[derive(Debug, Default)]
pub struct FactsScan {
    pub facts: Vec<StoredFact>,
    pub unreadable: usize,
}

/// 索引与树投影不是事实文件，一律跳过（`MEMORY.md` 由 `index.rs` 生成，`tree.md`
/// 是宿主 `memory_files` 写的投影）。
fn is_fact_file(p: &Path) -> bool {
    let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    p.extension().and_then(|s| s.to_str()) == Some("md") && stem != "MEMORY" && stem != "tree"
}

fn stored_from(path: &Path, parsed: &ParsedFact, workspace_fallback: &str) -> StoredFact {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    StoredFact {
        key: parsed.name.clone().unwrap_or(stem),
        workspace: parsed
            .workspace
            .clone()
            .unwrap_or_else(|| workspace_fallback.to_string()),
        value: parsed.body.trim_end().to_string(),
        tags: parsed.tags.clone().unwrap_or_default(),
        source: parsed
            .source
            .clone()
            .unwrap_or_else(|| "explicit".to_string()),
        updated_at: parsed.updated_at.unwrap_or(0),
        path: path.to_path_buf(),
    }
}

/// 按键位读一条。**文件不在 = `Ok(None)`**——"没有"不是错误，删除与查重都靠它
/// 区分（真源翻转后 `delete` 的 no-op 判据就是"文件在不在"，见 specs/05 刀② 取证④）。
/// 读得到字节就一律给读回（坏 frontmatter 走 `parse_fact` 的宽松兜底），只有
/// 真 IO 故障才 Err。
pub fn read_fact(root: &Path, workspace: &str, key: &str) -> Result<Option<StoredFact>, String> {
    let path = root.join(fact_rel_path(workspace, key));
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let fallback = if workspace.is_empty() {
                String::new()
            } else {
                super::project_dir_name(workspace)
            };
            Ok(Some(stored_from(
                &path,
                &sync::parse_fact(&text),
                &fallback,
            )))
        }
        Err(e) if fsutil::is_not_found(&e) => Ok(None),
        Err(e) => Err(format!("memory md 读取失败 {}: {e}", path.display())),
    }
}

/// 扫全根：全局层 + 每个项目层的 `facts/`。
///
/// 排序口径与现役 SQL **逐字同形**（`ORDER BY updated_at DESC, key`，宿主
/// `memory/crud.rs:28-29` 与 `recall.rs:38-40` 都是这条），所以读侧迁移那一步能拿
/// "既有测试一字未改仍绿"来担保显示顺序没变。
///
/// ⚠ 一处已知盲区（刀②b 接消费侧时必须定它，登记不偷偷改）：`unreadable` 现在只数
/// **读得到目录、读不动文件**那一层。`projects/` 本身读不动（非 NotFound 的 IO 故障）
/// 与某个项目层的 `facts/` 读不动都只被 `continue` 掉、不进账——因为"这一层从来没记过"
/// 与"这一层扫不动"在 `read_dir` 的返回值上分不开（前者也只是一个 NotFound）。
/// 后果：那种故障下 `facts` 为空而 `unreadable = 0`，消费侧看成的正是"没有"，
/// 违反召回侧那条纪律。要坐实它得让 `read_dir` 的 `Err` 参与记账（不分 NotFound），
/// 那是一次语义改动，不塞在本次"只添面不改行为"的刀里。
pub fn iter_facts(root: &Path) -> FactsScan {
    let mut scan = FactsScan::default();
    // 键位 → 该层目录反查兜底：全局层给 ''，项目层给目录名（不是权威原串）。
    let mut dirs: Vec<(PathBuf, String)> = vec![(root.join(GLOBAL_DIR), String::new())];
    if let Ok(rd) = std::fs::read_dir(projects_dir(root)) {
        for d in rd.flatten() {
            let facts = d.path().join("facts");
            if facts.is_dir() {
                let name = d.file_name().to_string_lossy().into_owned();
                dirs.push((facts, name));
            }
        }
    }
    for (dir, fallback) in dirs {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue; // 层不存在 = 该层没记过（不是故障，`global/` 就是惰性建的）
        };
        for f in rd.flatten() {
            let p = f.path();
            if !is_fact_file(&p) {
                continue;
            }
            match std::fs::read_to_string(&p) {
                Ok(text) => scan
                    .facts
                    .push(stored_from(&p, &sync::parse_fact(&text), &fallback)),
                Err(_) => scan.unreadable += 1,
            }
        }
    }
    scan.facts.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.key.cmp(&b.key))
    });
    scan
}
