//! InstructionFiles 单测：全局 + 项目两段拼装 / .Locowrk 回落 /
//! 缺文件 None / 快照锁字节 / 超限截断 / 开关关（`legacy`），三族六站的族序与
//! 族内二选一（`chain`），两档链预算与快照锁字节（`budget`）。

// 拆簇（v2.188 · 队列 Q4）：HEAD 版这个文件 136 行，本批把三族六站与两档链预算的
// 用例加进去之后越过 400 行 = `check_length` 报出新增超限件（23 > 基线 22）。
// 三问自检第①问也答「否」——它同时管「发现规则/族顺序」与「链预算」两件事，
// ⇒ 按 AGENTS 的规模纪律拆成目录式测试簇（不是为凑行数压注释，也不是抬基线）。
// helper 与夹具留在这里（三个子簇共用）。口径要分清：`legacy` 是**既有用例逐行搬移、
// 断言内容一字未改**（它们的原样在场 = 改前改后的等价证据）；`chain` / `budget`
// 两簇是本批新写的判据，不在"一字未改"的范围内。
mod budget;
mod chain;
mod legacy;

use super::InstructionFiles;
use crate::memory::MemoryStore;
use kernel_agent::InstructionSource;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn temp_dir(tag: &str) -> PathBuf {
    crate::testutil::unique_dir(&format!("instr-{}", tag))
}

/// 解析器：`ws-` 前缀会话 → 真实临时仓库（repo_root 要求 is_dir）。
fn fixture(tag: &str) -> (PathBuf, PathBuf, Arc<MemoryStore>) {
    let data = temp_dir(tag);
    let repo = data.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let store = MemoryStore::new(&data, {
        let repo = repo.clone();
        Arc::new(move |sid: &str| sid.starts_with("ws").then(|| repo.display().to_string()))
            as crate::workspace::WorkspaceResolver
    });
    (data, repo, Arc::new(store))
}

fn source(data: &Path, store: &Arc<MemoryStore>, on: bool) -> InstructionFiles {
    source_budgeted(data, store, on, 100, 100)
}

/// 固定两档预算的构造体。缺省传 100/100 = 改前那颗写死的 `MAX_FILE_BYTES`，
/// 所以「只有一份 AGENTS.md」的场景在改前改后逐字节相同（等价判据
/// `default_budgets_equal_the_old_hardcoded_cap` 钉的就是这个对账）。
fn source_budgeted(
    data: &Path,
    store: &Arc<MemoryStore>,
    on: bool,
    global_kib: u32,
    project_kib: u32,
) -> InstructionFiles {
    InstructionFiles::new(
        Arc::clone(store),
        data.join("AGENTS.md"),
        Arc::new(move || on) as Arc<dyn Fn() -> bool + Send + Sync>,
        Arc::new(move || (global_kib, project_kib)) as Arc<dyn Fn() -> (u32, u32) + Send + Sync>,
    )
}

// ===== v2.188 · 队列 Q4：三族候选 + 两档链预算 =====
// ⚠ 本族夹具的内容字符只用 `z` / `q`：截断与跳过两条标记句（"only N KB of the
// instruction chain budget was left" / "…already exhausted"）与 `# agentsMd` 头部
// 都不含这两个字母，所以「按字符计数」能当精确读数用。改前那条
// `oversize_file_truncated_with_marker` 用 `x`，靠的是头部句 "exactly" 里那个 x，
// 口径不同别混用。

/// 段落在链上的先后 = 模型读到的先后（越靠后权重越高）。这条只量**位置**：
/// 光断"两段都在场"分不清「override 排在最后」与「override 排在最前把 AGENTS
/// 挤掉了」——后者正是用户明确否掉的「同层替换」语义。
fn pos(hay: &str, needle: &str) -> usize {
    hay.find(needle).unwrap_or(usize::MAX)
}
