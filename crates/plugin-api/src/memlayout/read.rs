//! 事实文件的**读回与清点名**（S1 刀②a 立读回面，刀②c-1 加清空）：按键位取一条、
//! 扫全根、把整根清空——三种"从磁盘拿事实"的查法。
//!
//! 单独一片的理由：它管的是查法，跟渲染、写路径、索引各是不同件事。硬约束一条：
//! **frontmatter 只有 `sync::parse_fact` 一种读法**，这片不许自己再 split 一遍——
//! 两处解析同一格式迟早读出不一致的字段。
//!
//! 清点名（`iter_facts` / `clear_facts`）一律经 [`super::layers`] 问"有哪些层"，
//! 本片不自拼任何 `projects/<x>/facts` 路径——目录形状只许有一处算法。

use std::path::{Path, PathBuf};

use crate::fsutil;

use super::{fact_rel_path, layers, normalize_type, sync};
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
    /// `metadata.type`，**已经过 [`super::normalize_type`] 收档**：四类型之外的任何值
    /// （含旧文件的 `fact`、人乱写的、根本没这行的）都读成 [`super::UNCLASSIFIED_TYPE`]。
    /// 消费侧因此可以假定值域只有五种（四类型 + 未分类），不必自己再判一次。
    pub ty: String,
    /// metadata.updated（缺 / 坏 = 0）。
    pub updated_at: i64,
    pub path: PathBuf,
}

/// 一个键位目录的扫描结果。`unreadable` 是**没能扫出来的东西的条数**，两档同源计数：
/// ① 读得到目录、读不动某个事实文件（文件级）；② 整个层目录读不动 / 在场但不是
/// 目录（层级，S1 刀②c-1 起进账，理由见 [`iter_facts`]）。
///
/// 召回侧那条「库不可用不许冒充库是空的」的纪律（宿主 `memory/recall.rs`）搬到
/// 文件制之后就靠这个字段活着——空 `facts` 与"扫不动"必须能分开。
#[derive(Debug, Default)]
pub struct FactsScan {
    pub facts: Vec<StoredFact>,
    pub unreadable: usize,
}

/// 索引与树投影不是事实文件，一律跳过（`MEMORY.md` 由 `index.rs` 生成，`tree.md`
/// 是宿主 `memory_files` 写的投影）。
pub(crate) fn is_fact_file(p: &Path) -> bool {
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
        ty: normalize_type(parsed.ty.as_deref().unwrap_or_default()),
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
/// ⚠ 一处**先前登记为"故意不坐实"、刀②c-1 已坐实**的盲区（订正留痕）：
/// `unreadable` 过去只数"读得到目录、读不动文件"那一层，`projects/` 或某层 `facts/`
/// 本身读不动时只被 `continue` 掉 ⇒ 那种故障下 `facts` 空而 `unreadable = 0`，
/// 消费侧看成的正是"没有"。当时的理由是"没有跨平台可造的夹具"（POSIX `chmod 000`
/// 造得出、Windows 要动 ACL）。**这个理由被一条更普通的夹具推翻了**：在同一位置上
/// 放一个**普通文件**冒充目录，`read_dir` 两侧都失败（POSIX `ENOTDIR` / Windows
/// `ERROR_DIRECTORY`），且都不是 `NotFound` ⇒ 与"层不存在"天然可分。夹具不需要权限、
/// 不需要平台特判，所以这一档现在进账并有红证（`tests/read_back.rs` 两条）。
///
/// 为什么刀②c 必须补而不是继续挂着：sqlite 兜底档还在的时候，层级读不动最多让 md
/// 侧少一路、库里照样出数；真源翻转之后 md 是**唯一**读数来源，"整层扫不动"与
/// "从没记过"在用户面前就只剩同一个形状（管理页空白），而那正是这一刀要消灭的
/// 撒谎形态。
pub fn iter_facts(root: &Path) -> FactsScan {
    let mut scan = FactsScan::default();
    let (dirs, blocked) = layers(root);
    scan.unreadable = blocked;
    for layer in &dirs {
        let rd = match std::fs::read_dir(&layer.facts) {
            Ok(rd) => rd,
            // 层不存在 = 该层没记过（`global/` 是惰性建的），不是故障
            Err(e) if fsutil::is_not_found(&e) => continue,
            Err(_) => {
                scan.unreadable += 1;
                continue;
            }
        };
        for f in rd.flatten() {
            let p = f.path();
            if !is_fact_file(&p) {
                continue;
            }
            match std::fs::read_to_string(&p) {
                Ok(text) => scan
                    .facts
                    .push(stored_from(&p, &sync::parse_fact(&text), &layer.hint)),
                Err(_) => scan.unreadable += 1,
            }
        }
    }
    sort_facts(&mut scan.facts);
    scan
}

/// 一层的**磁盘读数**（S1 刀④c 管理页「每层一行」= ADR-0007 §18 那一行）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LayerStat {
    /// 层身份 = [`super::LayerDir::hint`]（`''` = 全局层，其余是 `<可读名>-<哈希>` 目录名）。
    pub hint: String,
    /// 读得出来的事实条数。
    pub count: usize,
    /// 该层最晚的一条 `metadata.updated`；一条都没有 = 0。
    pub newest_at: i64,
    /// **没能读出来的文件数**（文件级：目录读得动、某一篇读不动）。它与 `count` 分开记，
    /// 因为界面上的那个数字会被当成"我这层有几条记忆"——把读不动的算进 count 是虚报，
    /// 从读数里彻底抹掉它们是**瞒报**，两边都不许。
    pub unreadable: usize,
}

/// 按层给读数（一次扫描，不碰库）。第二返回值 = 没能扫出来的**层**数，与
/// [`iter_facts`] 的 `FactsScan::unreadable` 同一条口径（`layers()` 的第二个返回值
/// 加上文件级进账）。
///
/// ⚠ **扫不动的那一层不在这份清单里**：[`super::layers`] 只枚举读得动的目录（`facts/`
/// 被一个普通文件冒充时，那一层根本进不了 `dirs`）。所以调用方必须把第二个返回值交代
/// 出去——界面上是"另有 n 层读不到"那一行；把它当 0 处理，"读不到"就成了"没列出来"，
/// 那是瞒报（与召回侧"库不可用不许冒充库是空的"同族）。
///
/// 为什么单列一个读数、而不是让调用方拿 [`iter_facts`] 的结果按 `workspace` 分组：
/// 那一维在写了 frontmatter 的文件上是**权威原串**，在人自己新建的文件上是**目录名兜底**
/// （第五处 hint 陷阱）⇒ 按它分组会把同一层的东西拆成两堆。层的身份只认目录，所以这里
/// 直接按 [`super::layers`] 的 `LayerDir` 数 —— 界面上"显示哪一层 / 清哪一层 /
/// 开哪个文件夹"三件事从此指的是同一个 `LayerDir`。
///
/// 层目录不存在 = 这一层还没记过 ⇒ 仍然给出一行（`count = 0`）：全局层是惰性建的，
/// 一行"0 条"比"这一层凭空消失"诚实。
pub fn layer_stats(root: &Path) -> (Vec<LayerStat>, usize) {
    let (dirs, mut unreadable_layers) = layers(root);
    let mut out = Vec::with_capacity(dirs.len());
    for layer in &dirs {
        let mut st = LayerStat {
            hint: layer.hint.clone(),
            count: 0,
            newest_at: 0,
            unreadable: 0,
        };
        let rd = match std::fs::read_dir(&layer.facts) {
            Ok(rd) => rd,
            Err(e) if fsutil::is_not_found(&e) => {
                out.push(st);
                continue;
            }
            // 与 [`iter_facts`] 同一处置。到得了这一支的只能是"检查之后目录被人换掉"
            // 那种竞态：`layers()` 已经把非目录的层挡在 `dirs` 外面了。
            Err(_) => {
                unreadable_layers += 1;
                out.push(st);
                continue;
            }
        };
        for f in rd.flatten() {
            let p = f.path();
            if !is_fact_file(&p) {
                continue;
            }
            match std::fs::read_to_string(&p) {
                Ok(text) => {
                    st.count += 1;
                    st.newest_at = st
                        .newest_at
                        .max(sync::parse_fact(&text).updated_at.unwrap_or(0));
                }
                Err(_) => st.unreadable += 1,
            }
        }
        out.push(st);
    }
    (out, unreadable_layers)
}

/// 读**某一层**的全部事实（按键位算目录 ⇒ 只扫那一层，新→旧同一条排序口径）。
///
/// 为什么不让调用方拿 [`iter_facts`] 的结果按 `workspace` 字段自己过滤：那个字段在
/// 人新建的无 frontmatter 文件上是**目录名兜底**（见 [`super::LayerDir::hint`]），
/// 不是权威原串 ⇒ 按它过滤会把那个项目的文件漏掉、或错归到别的层去。按
/// [`super::scope_dirs`] 先算出目录再扫，拿到的才是"这一层"这个答案本身。
///
/// 层目录不存在 = 这一层还没记过 ⇒ 返回空表（不是故障）。读得动目录却读不动
/// 某个文件时**跳过那条**（与 `iter_facts` 同一处置）；调用方要的是清单，
/// 一条坏文件不该把整层报成不可用——那一维由 [`iter_facts`] 的计数负责。
pub fn facts_of(root: &Path, workspace: &str) -> Result<Vec<StoredFact>, String> {
    let (dir, _) = super::scope_dirs(root, workspace)?;
    let rd = match std::fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(e) if fsutil::is_not_found(&e) => return Ok(Vec::new()),
        Err(e) => return Err(format!("memory 层读取失败 {}: {e}", dir.display())),
    };
    let mut out = Vec::new();
    for f in rd.flatten() {
        let p = f.path();
        if !is_fact_file(&p) {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(&p) {
            out.push(stored_from(&p, &sync::parse_fact(&text), workspace));
        }
    }
    sort_facts(&mut out);
    Ok(out)
}

/// 唯一的排序口径（`updated_at DESC, key ASC`）。
///
/// 与现役 SQL 逐字同形的理由见 [`iter_facts`]；抽成函数是因为现在有两个消费者
/// （全根扫描与单层读取），各写一份闭包迟早漂成两种顺序。
fn sort_facts(facts: &mut [StoredFact]) {
    facts.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.key.cmp(&b.key))
    });
}

/// 子串匹配的**唯一**口径：`key` / `value` / `tags` 三维，ASCII 大小写不敏感，
/// 通配符按字面量。
///
/// 为什么必须只有一处：这条谓词有两个消费者（宿主管理页的列举过滤与插件
/// `memory_search`），而它们今天读的是**同一份文件**。各写一遍迟早漂成
/// "管理页搜得到、工具搜不到"（或反过来），那种漂移在界面上表现为"记忆丢了"。
///
/// SQL 时代那句 `LIKE ? ESCAPE '\'` 的转义只为语法层服务（把用户数据里的 `%` `_`
/// 从通配符降级成字面字符）；文件侧没有语法层，字面量就是 `contains`。
/// 大小写折叠同样照 SQL 侧口径只折 ASCII——POSIX 上 `LIKE` 对非 ASCII 也是敏感的，
/// 折全 Unicode 会让 Linux 与 Windows 搜出不同结果。
pub fn fact_matches(f: &StoredFact, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let n = needle.to_ascii_lowercase();
    [f.key.as_str(), f.value.as_str(), f.tags.as_str()]
        .into_iter()
        .any(|s| s.to_ascii_lowercase().contains(&n))
}

/// 清空真源里全部事实文件（逐层删 + 刷该层索引），返回**删掉的条数**。
///
/// 为什么住在这片、而不是让宿主自己遍历再逐条 `delete_fact`：只有这里知道
/// "哪些文件算事实"（[`is_fact_file`] 排 `MEMORY.md` / `tree.md`），也只有
/// [`super::layers`] 知道层的目录形状。宿主若按扫到的行反推 workspace 再算一次路径，
/// 会在"人新建的无 frontmatter 文件"上拿到**目录名兜底**当键位 ⇒ 指到别的目录、
/// 删不到也不报错（`LayerDir::hint` 那句警告就是这个坑的登记处）。
///
/// 扫不动的层**中止并 Err**，而不是静默少删：清空是个要报数的动作，"说清了其实
/// 没清"比失败更难发现。半路 Err 时前面几层已经删了——清空不是事务，这一点如实
/// 上抛给调用方（管理页据此报错、不显示成功计数）。
pub fn clear_facts(root: &Path) -> Result<usize, String> {
    let (dirs, blocked) = layers(root);
    if blocked > 0 {
        return Err(format!(
            "memory 真源有 {blocked} 个层目录扫不动（清空中止，未改动任何文件）"
        ));
    }
    let mut n = 0usize;
    for layer in &dirs {
        let rd = match std::fs::read_dir(&layer.facts) {
            Ok(rd) => rd,
            Err(e) if fsutil::is_not_found(&e) => continue,
            Err(e) => {
                return Err(format!(
                    "memory 清空时读不动 {}: {e}",
                    layer.facts.display()
                ));
            }
        };
        let doomed: Vec<PathBuf> = rd
            .flatten()
            .map(|f| f.path())
            .filter(|p| is_fact_file(p))
            .collect();
        for p in doomed {
            match std::fs::remove_file(&p) {
                Ok(()) => n += 1,
                Err(ref e) if fsutil::is_not_found(e) => {}
                Err(e) => return Err(format!("memory 清空删不掉 {}: {e}", p.display())),
            }
        }
        super::index::refresh_layer(&layer.facts, &layer.index)?;
    }
    Ok(n)
}
