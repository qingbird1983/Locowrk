//! 记忆人类层的**寻址面**（S1 刀②c-1 自 `mod.rs` 整块拆出，**逐字未改**）：
//! 目录名、项目目录算式、键位 → 相对路径、总开关 env，以及"真源里有哪些层"。
//!
//! 为什么这时候拆它：刀②c-1 给这一面加了「按磁盘在场枚举层」（`layers`），`mod.rs`
//! 涨到 416 行越过 400 纪律。三问自检：① 它只管一件事吗——是（"东西落在哪个路径"）；
//! ② 名字概括得住吗——`paths` 正是这句话；③ 改它常牵动无关部分吗——不会（渲染与写删
//! 只在已经算好的路径上落字节）。三问皆是 ⇒ 按 AGENTS §3 本可不拆并把理由写进提交正文，
//! 但这一面本来就与"渲染格式"是两种改法（改名字 = 改磁盘形状，改渲染 = 改文件内容），
//! 拆了以后两边各自演化互不牵连 ⇒ 选择拆。
//!
//! ⚠ 拆完之后 `check-single-source` 的 **SS1b 口径必须跟着改**（它原先写死扫
//! `memlayout/mod.rs` 单文件，常量搬到本文件后左值 2→0 报了一条"定义丢了"的假红——
//! 与刀②a 时 `SS10b` 撞过的同一个洞，那次学到的教训就是"扫描面吃子树不吃单文件"）。

use std::path::{Path, PathBuf};

use crate::fsutil;

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
/// 全局层的目录名。**与上面 `PROJECTS_DIR` 同一条纪律**：人类层里 `"global"` 这个
/// 字面量在本模块只许出现在这一处——写侧两条（`fact_rel_path` 的相对支与
/// `scope_dirs` 的绝对支）与读侧（`read::iter_facts`）全部经这里取，多一个消费者
/// 也不会散出第二份抄本（索引预算那两个数字同理住在 `index.rs`）。
/// ⚠ 宿主侧还有两处字面量在 `memory_reconcile.rs`（该模块在 v1.1 里整项作废、待退役），
/// 收口跟那一刀一起做，已登记进 specs/05 挂账——本刀不顺手搬。
pub(crate) const GLOBAL_DIR: &str = "global";
/// 总开关 env。**S1 刀③b 之后本仓没有任何生产读者了**：它的最后一个读者是插件那侧
/// "sqlite 成功后同步 md"，而六枚工具现在直接写真源——"关掉就不落文件"这件事在
/// 真源 = 文件的世界里等于"关掉就没有记忆"，不是同一个开关。宿主也不再注入它
/// （`state/boot.rs` 与 `commands/settings.rs` 那两处随本刀摘掉）。
///
/// 为什么这一对函数还留着而不是顺手删：`files_enabled` 是 `plugin-api` 的**既有对外
/// 签名**，删它属 AGENTS §5 禁区（要退役得先向人申请并说明）。它的判据
/// `enabled_from` 仍是纯函数单测的对象（`tests/layout.rs`）。登记在 specs/05 挂账。
const ENV_FILES: &str = "LOCOWRK_MEMORY_FILES";

/// 总开关判定（纯函数，可测）：`Some("0")` = 关，其余（含 None）= 开。
pub(crate) fn enabled_from(v: Option<&str>) -> bool {
    v != Some("0")
}

/// 当前进程的文件化总开关（**自刀③b 起无人调用**，见 [`ENV_FILES`] 上面那段）。
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

/// stem 的**字符**上限。为什么布局层要管这件事：工具的参数面允许 key 长到 512 字节
/// （原型 memory-sqlite 插件的 `KEY_MAX`，写死给模型看的；插件已随 S4 刀③c 删除，
/// 此值作为布局常量保留），
/// 而 Windows 的路径上限是 260 字符——S1 刀③b 把六枚工具扳成"直接写文件"之后当场撞到：
/// 512 个 `k` 的 key 得到 `os error 123`（文件名、目录名或卷标语法不正确）⇒
/// **一条合法的记忆写不进去**。
///
/// 这一维由布局吸收而不是把 `KEY_MAX` 降到 100：降上限 = 改工具参数面（模型与既有会话
/// 历史看到的契约跟着变），而"文件系统有多长"从来是实现细节。手法与
/// [`project_dir_name`] 同源——可读前缀 + 全串 fnv 短哈希，确定性、不撞、
/// 人打开文件夹仍然认得出是哪条。
const STEM_MAX_CHARS: usize = 100;

/// 超上限的 stem 收口：前 91 字符 + `-` + 完整 stem 的 fnv 高 32 位 hex8 = 恰好 100。
fn bound_stem(s: &str) -> String {
    if s.chars().count() <= STEM_MAX_CHARS {
        return s.to_string();
    }
    let head: String = s.chars().take(STEM_MAX_CHARS - 9).collect();
    format!("{head}-{:08x}", (fnv1a64(s.as_bytes()) >> 32) as u32)
}

/// key → 文件名主干：保留 `[A-Za-z0-9._-]`、其余换 `_`，端点 `.` 剥除，
/// Windows 保留设备名（con/prn/aux/nul/comN/lptN）前缀 `_` 防创建失败，
/// 最后按 [`STEM_MAX_CHARS`] 收口成长度有界的形态（见那条注释里的实测缺陷）。
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
    // 保留名防护在前、长度收口在后。这个次序是有意的：`_` 前缀那一条只在短名字上发生，
    // 而截断只会发生在几十字符以上——反过来的话，一条以 `con` 开头又长得离谱的 key
    // 会在截断之后丢掉防护又或者是白加一个前缀。
    let guarded = if looks_reserved {
        format!("_{stem}")
    } else {
        stem
    };
    bound_stem(&guarded)
}

/// 键位 → 相对路径：全局 `''` = `global/<stem>.md`；项目 =
/// `projects/<dir>/facts/<stem>.md`（项目支一律经 `project_rel_dir`）。
pub fn fact_rel_path(workspace: &str, key: &str) -> PathBuf {
    let file = format!("{}.md", key_file_stem(key));
    if workspace.is_empty() {
        PathBuf::from(GLOBAL_DIR).join(file)
    } else {
        project_rel_dir(workspace).join("facts").join(file)
    }
}

/// 键位目录与索引文件：全局 = `global/`，项目 = `projects/<dir>/facts/`
/// （索引坐在项目文件夹根）。项目支经 `project_dir` ⇒ `''` 在这里同样**拒**，
/// 由调用点决定怎么办（`refresh_index` 直接上抛；reconcile 记 warning 跳过）。
/// `pub(crate)` 是给 `index.rs`（写索引）与 `read.rs`（读一层）共用的——目录形状
/// 只这一处算法，两片各自再拼一遍就是第二个出处。
///
/// ⚠ 与 [`layers`] 的分工：这里按**键位串**算一层（写侧：我知道要写哪个项目）；
/// `layers` 按**磁盘在场**枚举全部层（读侧与清空：手里只有根，没有键位清单）。
/// 两者必须都存在，是因为项目层的 workspace 原串**只能从权威数据里拿**——文件制
/// 里人新建的文件没写 frontmatter，从目录名反推不出原串。
pub(crate) fn scope_dirs(root: &Path, workspace: &str) -> Result<(PathBuf, PathBuf), String> {
    if workspace.is_empty() {
        let dir = root.join(GLOBAL_DIR);
        Ok((dir.clone(), dir.join("MEMORY.md")))
    } else {
        let dir = project_dir(root, workspace)?;
        Ok((dir.join("facts"), dir.join("MEMORY.md")))
    }
}

/// 真源里的一层：扫事实的目录 + 它的索引文件 + 给人看的层名。
pub struct LayerDir {
    /// 事实文件所在目录（全局 = `global/`，项目 = `projects/<dir>/facts/`）。
    pub facts: PathBuf,
    /// 该层的 `MEMORY.md`（项目层坐在项目文件夹根，与 `facts/` 同级）。
    pub index: PathBuf,
    /// 兜底身份：全局层 = `""`，项目层 = 那个 `<可读名>-<哈希>` 目录名。
    /// ⚠ **不许拿它寻址**——它不是权威 workspace 原串（反推不出来），只给人看。
    /// 拿它再去算 `fact_rel_path` 会指到另一个目录，这是刀②c-1 实测踩到的坑。
    pub hint: String,
}

/// 按磁盘在场枚举全部层（S1 刀②c-1 起这是"有哪些层"的**唯一**答案：读侧
/// `read::iter_facts` 与清空 `read::clear_facts` 都吃它，各自再拼一遍
/// `projects/<x>/facts` 就是第二个出处）。
///
/// 返回 `(层, 扫不动的目录数)`。**后一半不是可选的装饰**：只返回 Vec 的话，
/// `projects/` 整层读不动与"一个项目都没记过"是同一个答案（空 Vec），而真源 = 文件
/// 之后没有别的源能揭穿它。计数非零时调用方必须交代出去（读侧进 `FactsScan.unreadable`
/// ⇒ 召回 `partial` ⇒ 埋点不记；清空中止并 Err）。
/// 夹具 = 在同一位置放一个普通文件冒充目录，两侧 `read_dir` 都失败且都不是
/// `NotFound`（`read_back.rs` 两条用例）。
pub fn layers(root: &Path) -> (Vec<LayerDir>, usize) {
    let global = root.join(GLOBAL_DIR);
    let mut out = vec![LayerDir {
        index: global.join("MEMORY.md"),
        facts: global,
        hint: String::new(),
    }];
    let mut blocked = 0usize;
    match std::fs::read_dir(projects_dir(root)) {
        Ok(rd) => {
            for d in rd.flatten() {
                let facts = d.path().join("facts");
                if facts.is_dir() {
                    out.push(LayerDir {
                        index: d.path().join("MEMORY.md"),
                        hint: d.file_name().to_string_lossy().into_owned(),
                        facts,
                    });
                } else if facts.exists() {
                    // 在场但**不是目录**（被文件冒充）⇒ 这个项目的事实层一条也扫不出来。
                    blocked += 1;
                }
            }
        }
        // `NotFound` = 还没有项目层，不是故障；其余（不是目录 / 权限 / IO）= 整层没能扫出来。
        Err(e) if fsutil::is_not_found(&e) => {}
        Err(_) => blocked += 1,
    }
    (out, blocked)
}
