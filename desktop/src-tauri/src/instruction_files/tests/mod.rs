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
use crate::workspace::WorkspaceResolver;
use kernel_agent::InstructionSource;
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn temp_dir(tag: &str) -> PathBuf {
    crate::testutil::unique_dir(&format!("instr-{}", tag))
}

/// 解析器：`ws-` 前缀会话 → 真实临时仓库（repo_root 要求 is_dir）。
fn fixture(tag: &str) -> (PathBuf, PathBuf, WorkspaceResolver) {
    let data = temp_dir(tag);
    let repo = data.join("repo");
    std::fs::create_dir_all(&repo).unwrap();
    let ws: WorkspaceResolver = {
        let repo = repo.clone();
        Arc::new(move |sid: &str| sid.starts_with("ws").then(|| repo.display().to_string()))
    };
    (data, repo, ws)
}

/// 生产形态：恒定开启、单链 100 KiB（S4 刀③c 写死的 [`super::INSTRUCTION_CHAIN_KIB`]）。
fn source(data: &Path, ws: &WorkspaceResolver) -> InstructionFiles {
    InstructionFiles::new(Arc::clone(ws), data.join("AGENTS.md"))
}

/// 小额度构造体：只服务链装配体的截断/跳过标记判据——生产没有第二个档位了。
fn source_with_cap(data: &Path, ws: &WorkspaceResolver, cap_kib: usize) -> InstructionFiles {
    InstructionFiles::with_cap(Arc::clone(ws), data.join("AGENTS.md"), cap_kib * super::KIB)
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
