//! 指令文件层宿主实现（提示词分层设计 §4.3）：全局文件取 `<数据根>/AGENTS.md`，
//! 项目层认**三族**候选名（`AGENTS.md` / `CLAUDE.md` / `AGENTS.override.md`），
//! 拼成 [`InstructionSource`] 内文，由内核
//! [`kernel_agent::InstructionFilesInject`] 包裹 `<system-reminder>` 后插
//! 消息列头部。
//!
//! **族内二选一，族间串联**（v2.188 · 队列 Q4）：同一约定名的两个家（仓库根与
//! `.Locowrk/`）只取第一个能读到内容的——双读会把同一份约定注两遍；不同约定名之间
//! 则**都注入**，且顺序 = 模型读到的顺序 = 越靠后权重越高，所以 override 族排在最后
//! （用户 2026-10-13 裁定的「追加不替换」：override 在场时同族的 `AGENTS.md` 仍然注入，
//! override 只是补在后面，不屏蔽同层任何东西）。
//!
//! **S4 刀③c（AC-7）起本层恒定开启、预算写死 [`INSTRUCTION_CHAIN_KIB`]**：
//! 两条链各 100 KiB，100 就是旧设置项的缺省值，所以「项目里只有一份 AGENTS.md」
//! 这个存量主形态与改前逐字节相同。指令文件不是记忆——它注的是人在仓库里写下的
//! 约定，与记忆库无关，因此不随记忆整线退役；而"开关 + 两档预算"三枚设置项
//! 属于用户裁定里"不该让普通用户填数值"的那一类，收进代码。
//! 额度用尽时后面的文件**不读并留一句如实说明**，装不满的文件截断并标注当前
//! 上限——「指令文件写坏了如实呈现，不静默丢弃」（分层稿 §9.3）。
//!
//! 快照锁字节（设计稿 §3.3）：按 `(session, scope)` 键位**会话首查读文件并缓存**
//! （None 也缓存），中途人改文件 = 显式变更：当前会话不变，下一会话生效；
//! 会话改归属 = 换键 = 重算。全局文件缓存在 `""` 会话键位下进程级共享（读一次）。

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use kernel_agent::InstructionSource;

use crate::workspace::{scope_key, WorkspaceResolver};

/// KiB → 字节（预算按 KiB 表述，读盘按字节）。
const KIB: usize = 1024;

/// 每条链的字节预算（S4 刀③c 写死，AC-7）：两条链各 100 KiB。
/// 100 是旧设置项 `instructionGlobalKib` / `instructionProjectKib` 的缺省值，
/// 改前改后的存量主形态逐字节相同。
pub(crate) const INSTRUCTION_CHAIN_KIB: usize = 100;

/// 缓存上限（FIFO 驱逐）：与记忆预取同款的保险丝（那个实现已随记忆线退役，
/// 带上限的理由不变——会话表无界增长时缓存不能跟着无界）。
const CACHE_CAP: usize = 128;

/// 项目层族号（族内二选一、族间串联的分组键）。顺序即优先级：越靠后模型读得越晚
/// = 权重越高，所以 override 是 2 而不是 0。
const FAM_AGENTS: usize = 0;
const FAM_CLAUDE: usize = 1;
const FAM_OVERRIDE: usize = 2;

/// `CLAUDE.md` 的回落目录名（Claude Code 的项目内配置面）。⚠ 这与
/// `llm/import/claude.rs` 里那个 `~/.claude/settings.json` 的**用户主目录**同名但
/// 不同概念（一个是仓库内目录、一个是 home 下目录），不合并成一个常量——
/// 强行统一会造出一条假耦合：改"项目回落目录"会连带改到供应商配置导入路径。
const CLAUDE_DIR: &str = ".claude";

/// Locowrk 的项目内配置目录名（`AGENTS.md` 等三族的第二站）。S4 刀③c 之前借自
/// `repo_context::DIR`；那个模块随记忆线退役后，唯一消费者就是本文件。
pub(crate) const LOCAL_DIR: &str = ".Locowrk";

/// 指令文件源（组装点构造：全局路径 = 数据根 AGENTS.md；项目根 = 会话旁车
/// workspace，经 [`WorkspaceResolver`] 每次调用现查）。
pub struct InstructionFiles {
    workspace_of: WorkspaceResolver,
    global_path: PathBuf,
    /// 单链字节额度。生产 = [`INSTRUCTION_CHAIN_KIB`] × KiB；测试可注入小额度
    /// 以验链装配体的截断/跳过两条标记。
    cap: usize,
    cache: Mutex<Cache>,
}

#[derive(Default)]
struct Cache {
    map: HashMap<String, Option<String>>,
    order: VecDeque<String>,
}

impl InstructionFiles {
    pub fn new(workspace_of: WorkspaceResolver, global_path: PathBuf) -> Self {
        Self::with_cap(workspace_of, global_path, INSTRUCTION_CHAIN_KIB * KIB)
    }

    fn with_cap(workspace_of: WorkspaceResolver, global_path: PathBuf, cap: usize) -> Self {
        Self {
            workspace_of,
            global_path,
            cap,
            cache: Mutex::new(Cache::default()),
        }
    }

    /// 归属仓库根：旁车 workspace 且目录在场；未归属 = 项目链整条不做。
    fn repo_root(&self, session_id: Option<&str>) -> Option<PathBuf> {
        let sid = session_id?;
        let ws = (self.workspace_of)(sid)?;
        if ws.is_empty() {
            return None;
        }
        let repo = PathBuf::from(&ws);
        repo.is_dir().then_some(repo)
    }

    /// 缓存键里的 scope 段：与 workspace 键位同一归一（`scope_key`）。
    fn scope_of(&self, session_id: Option<&str>) -> String {
        scope_key(session_id.and_then(|s| (self.workspace_of)(s)).as_deref())
    }

    /// 未命中路径的加载体（纯函数语义：读盘 + 拼段，无隐藏状态）。
    fn load_uncached(&self, session_id: Option<&str>) -> Option<String> {
        let mut sections: Vec<String> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        // 全局链：一个位置，但同样走链装配体（预算的口径是「链」而不是「文件」，
        // 两个成分共用一段代码才不会各自漂）。
        fill_chain(
            &[(0usize, self.global_path.clone())],
            self.cap,
            &mut sections,
            &mut notes,
        );
        // 项目链：归属会话的仓库根，三族候选按族号分组。
        if let Some(repo) = self.repo_root(session_id) {
            fill_chain(
                &project_candidates(&repo),
                self.cap,
                &mut sections,
                &mut notes,
            );
        }
        if sections.is_empty() {
            return None;
        }
        let mut body = format!(
            "# agentsMd\nCodebase and user instructions are shown below. Be sure \
             to adhere to these instructions. IMPORTANT: These instructions \
             OVERRIDE any default behavior and you MUST follow them exactly as \
             written.\n\n{}",
            sections.join("\n\n")
        );
        // 被额度挤掉的文件**必须说出来**：静默少注一个族的约定 = 模型以为自己拿到了
        // 全部项目规则（与截断同一口径，分层稿 §9.3）。
        // ⚠ `push_str("\n\n")` 是**两**字符字面量：clippy 的 `single_char_add_str`
        // 会把单字符 `push_str("\n")` 判错，而这里要的是"段与段之间空一行"的双换行。
        for n in notes {
            body.push_str("\n\n");
            body.push_str(&n);
        }
        Some(body)
    }
}

impl InstructionSource for InstructionFiles {
    fn load(&self, session_id: Option<&str>) -> Option<String> {
        // 缓存键 = (session, scope)：归属变更 = 换键 = 重算（㉘·A·fix
        // 同款键位纪律；全局文件在 ("", scope) 下照常缓存——scope 空 = 键
        // 同，命中进程级一份）。
        let key = format!(
            "{}\u{1}{}",
            session_id.unwrap_or(""),
            self.scope_of(session_id)
        );
        let mut cache = self.cache.lock().expect("instruction cache poisoned");
        if let Some(hit) = cache.map.get(&key) {
            return hit.clone();
        }
        let loaded = self.load_uncached(session_id);
        if !cache.map.contains_key(&key) {
            cache.order.push_back(key.clone());
            while cache.order.len() > CACHE_CAP {
                if let Some(old) = cache.order.pop_front() {
                    cache.map.remove(&old);
                }
            }
        }
        cache.map.insert(key, loaded.clone());
        loaded
    }
}

/// 项目层候选表：`(族号, 路径)`，**同族内按给出的先后排**（先者 = 仓库根），
/// 族与族之间按本函数给出的顺序串联（AGENTS → CLAUDE → override）。
/// ⚠ 两个回落目录名都走常量（本文件的 [`LOCAL_DIR`] / `CLAUDE_DIR`），
/// 不在候选表里重复字面串——「同一属性还剩几个出处」才是修好了的机器形状。
fn project_candidates(repo: &Path) -> Vec<(usize, PathBuf)> {
    vec![
        (FAM_AGENTS, repo.join("AGENTS.md")),
        (FAM_AGENTS, repo.join(LOCAL_DIR).join("AGENTS.md")),
        (FAM_CLAUDE, repo.join("CLAUDE.md")),
        (FAM_CLAUDE, repo.join(CLAUDE_DIR).join("CLAUDE.md")),
        (FAM_OVERRIDE, repo.join("AGENTS.override.md")),
        (
            FAM_OVERRIDE,
            repo.join(LOCAL_DIR).join("AGENTS.override.md"),
        ),
    ]
}

/// 一条链的装配体：族内先到先得（同族第二个位置不再读），族间按给出顺序串联；
/// `cap` 是这条链的总字节额度。装不进剩余额度的**截断 + 标注当前上限**，
/// 额度归零后还在排队的文件**跳过并留一句说明**——两者都不静默。
/// ⚠ 缺文件 / 空文件 **不算族命中**（继续试同族下一个位置），与改前行为一致。
fn fill_chain(
    candidates: &[(usize, PathBuf)],
    cap: usize,
    sections: &mut Vec<String>,
    notes: &mut Vec<String>,
) {
    let mut taken: Vec<usize> = Vec::new();
    let mut used = 0usize;
    for (family, path) in candidates {
        if taken.contains(family) {
            continue;
        }
        let Ok(bytes) = std::fs::read(path) else {
            continue;
        };
        if bytes.is_empty() {
            continue;
        }
        taken.push(*family);
        if used >= cap {
            notes.push(format!(
                "[Skipped {}: the instruction chain budget of {} KB is already exhausted]",
                path.display(),
                cap / KIB
            ));
            continue;
        }
        let room = cap - used;
        let truncated = bytes.len() > room;
        let mut text = String::from_utf8_lossy(&bytes[..bytes.len().min(room)]).into_owned();
        if truncated {
            // 标注带的是**这个文件实际拿到的额度**（剩余额度），不是整条链的档位——
            // 否则同一条链里第二个被截的文件报 100 KB 而只装进 4 KB，标记在说谎。
            text.push_str(&format!(
                "\n\n[File truncated: only {} KB of the instruction chain budget was left]",
                room / KIB
            ));
        }
        used += text.len();
        sections.push(section(&path.display().to_string(), text));
    }
}

/// 单文件段标题（ZCode agentsMd 同款形状）。
fn section(path: &str, text: String) -> String {
    format!("Contents of {path} (workspace instructions):\n\n{text}")
}

#[cfg(test)]
mod tests;
