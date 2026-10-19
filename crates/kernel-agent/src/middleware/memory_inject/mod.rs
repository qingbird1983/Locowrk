//! Memory-Inject（Context and Memory.md §4.2.3 / §5 + 方案稿 §C）：`agent/request`
//! waterfall 监听器——把**两层记忆索引的指针行**注入请求级 system prompt
//! （`RequestPayload::system`），正文一个字都不注入。
//!
//! **注入面（S3 刀① 起 = 「索引常驻，正文按需」）**：一个 `<memory_index>` 块装两段
//! ——全局层 `MEMORY.md` 与本项目 `MEMORY.md` 的指针行（`- [标题](文件.md) — 钩子`）。
//! 模型由此知道"记了哪些条、各在哪一篇"，要用哪条就自己读那篇文件。它取代的是旧形状
//! 那两块：`<persistent_memory>`（全局层事实**正文** top-K，还挂一条按 sqlite 加权的
//! 排序档）与 `<project_memory_index>`（只有项目层索引）。方案稿 §C 那句"并进同一处、
//! 消灭双出处"指的就是这两块各说一套。
//!
//! 这不是对 v2.136「项目侧全提醒制」的反转：那条禁的是把事实全文灌进 system，而索引给
//! 的是"这个项目/这个用户记过哪些条"，正文照旧按需读——也正是 MR 稿 §0 那条真实失败的
//! 对症件（注入面三重盲：库里有货而模型不知道有货）。
//!
//! 链位（§3.6；v2.139 提示词分层设计 §6 重排）：ToolExposure（⑫）→
//! BaseSystemInject（静态身份层 = system 前缀）→ **Memory-Inject** →
//! 思考语言 → 树目录（殿后）——静态段在前、动态块追加，append-only
//! 目录永远最后。上下文压缩件（ctx-compact）自 v2.137 桌面链退役、并随
//! 审计修复批 C 删除本体（headless 侧其实从未注册过它），节奏改由记忆树
//! 的整理边界承担。
//!
//! 系统提示永不压缩（§5.3 冲突 2）由结构保证：记忆写在消息列表之外的
//! `system` 字段里，任何改写消息的决策方（历史上是 ctx-compact）都只动
//! `messages`，会话日志也不落注入内容——投影零改动。
//!
//! 与 mw-audit 同理落在宿主侧：事实记忆的真源是 md 文件（S1 刀③ 之后连插件都不开库，
//! `memory-sqlite` 这个 ID 只拥有那六枚工具的入口），内核不碰存储——检索经
//! [`MemorySource`] trait（组装点提供
//! 实现，与 ApprovalResolver 同款"内核定契约、组装点实现"模式；不经
//! ToolSource 走工具管道，否则每 step 一次 memory_list 会污染 ⑩ 的使用
//! 统计与审计日志）。kernel 每 step 从会话重建 payload，`system` 恒从
//! None 起算，天然无重复注入。
//!
//! 会话级预取（KV v2 纪律 2，M1）：逐轮动态检索是缓存头号杀手——记忆库
//! 中途写入会改写 system 前缀使命中归零。带会话身份的请求在**首次注入
//! 时检索一次并缓存块文本**，后续轮复用同一字节。top_k 配置变更 = 显式变更，
//! 重算接受失效。缓存键 = `(session_id, scope_id, index_stamp)`
//! （㉘·A·fix 起前两段；S3 刀② 加第三段）：键位不变**且注入面的输入文件没动**
//! 才逐字节复用。无会话身份 = 每轮检索（一次性进程等价会话，headless 语义不变）。
//!
//! ⚠ **刀② 是一次语义放宽，代价要说破**（不许把它说成"延续 M1 纪律"）：
//! 旧形状下本会话内索引字节恒冻结，`memory_write` 中途写库也看不见自己写了什么；
//! 新形状下"文件 mtime 变"就重算 ⇒ 于是**分不清改的是人还是模型**，模型自己写一条
//! 事实之后，下一轮会付一次前缀缓存断裂。买到的东西：人编辑 `MEMORY.md` 当场生效
//! （§C/§F 的硬要求）+ 模型新记的那条本会话内可见。付掉的东西：每次写入后一轮全价
//! input（与 S2 刀④ 主层压缩那种"一次性前缀重置"同量级，不是每轮都付）。

use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use cordis_core::{WaterfallDecision, WaterfallListener};
use serde_json::Value;

use crate::waterfall::{append_block, points, take_request};

mod contract;

pub use contract::{LearnWeights, MemoryEntry, MemoryInjectConfig, MemorySource, RecallQuery};

/// 单条 value 注入上限（字符）：记忆库单条可达 256KiB，原样注入一轮就
/// 顶爆窗口；超长截断并标注（全文在记忆管理页可看）。
///
/// ⚠ **S3 刀① 之后它已经不在注入面上**：`<persistent_memory>` 那块正文被索引段取代，
/// 本仓今天唯一还在用它的是宿主侧的 `relevance` 匹配面（见下一段那条纪律）。它的去留
/// 随刀③（`memory_learn` 一族按裁定同刀退役 ⇒ 那时这颗常量彻底无使用者）。
///
/// **`pub` 的理由不是"方便"而是"匹配面必须等于注入面"**（队列 Q9）：宿主侧的
/// `relevance` 信号要对 value 做关键词匹配，如果它扫全文，一条记忆会**因为模型
/// 根本看不到的第 8000 字命中而排到前面**——那是在给"注入块里没有的东西"打分。
/// 所以宿主复用这同一个常量截断匹配面；本仓判据要求宿主侧不得再出现 `500`
/// 字面量（同一属性两个出处 = 历批立的纪律在这里正适用）。
pub const VALUE_INJECT_MAX_CHARS: usize = 500;

/// 预取缓存上限（FIFO 驱逐最旧会话）：与 ToolExposure 钉死表同款保险丝。
const PREFETCH_CAP: usize = 128;

/// 主动记忆指引（F3 起随注入块尾；v2.139 按提示词分层设计 §5.2 改写；
/// **Q2 · MR §4.4 增两句并给禁令开一个显式口子**）。
///
/// 分工不变（v2.136 裁定）：write 侧保持主动（当场记，不等被问）；read 侧改按需。
/// 两条 Q2 增句的由来 = MR 稿 §0 的实测动线：① 旧措辞「never as a way to
/// start a conversation」被模型读成"关于过去的提问也别查"，于是用户问"查一下你
/// 之前的记忆"时它**直接从注入上下文假设**、一个工具都不调 → 现在明确"用户显式
/// 问过去/记忆时**必须查工具**"；② 事实层空是常态（0 行）而**树里真有货**，模型
/// 查完 `memory_search` 落空就下结论"没有历史" → 现在明说"空结果 ≠ 没有历史，
/// 项目历史在树里，先 `memory_project_history`"。
///
/// memory 注入关（`memory.enabled=false` / top_k=0）= 整块含指引一起缺席。
/// 静态文本与块同吃预取缓存（M1 纪律不破）。
const GUIDANCE_BLOCK: &str = "<memory_guidance>\n\
     Proactive memory: when the user states a lasting preference or a durable \
     decision is made, save it with memory_write on the spot (scope=\"global\" \
     for standing preferences; project scope for durable project facts); \
     prefer updating an existing key over near-duplicates. Reading memory is \
     on-demand: use memory_read / memory_search only when the request touches \
     past preferences, decisions, or project facts — don't probe memory \
     unprompted as a way to start a conversation. When the user explicitly asks \
     about past work or memories, query these tools instead of assuming from \
     injected context. Empty results from memory_read / memory_search do NOT \
     mean there is no history: project history lives in the memory tree — call \
     memory_project_history first, and memory_search_titles for topics. Project \
     working memory (.Locowrk/ files) you maintain with file tools; the repo \
     context reminder says when.\n\
     </memory_guidance>";

/// 一个会话的预取块（None = 首查即无记忆，也缓存避免每轮重查）。
#[derive(Debug, Clone)]
struct PrefetchEntry {
    block: Option<String>,
    top_k: usize,
}

/// 会话 → 预取块表（FIFO 驱逐）。
#[derive(Default)]
struct Prefetch {
    map: std::collections::HashMap<String, PrefetchEntry>,
    order: std::collections::VecDeque<String>,
}

impl Prefetch {
    /// 命中且 top_k 未变才复用（top_k 变更 = 显式配置变更，重算）。
    fn get(&self, id: &str, top_k: usize) -> Option<Option<String>> {
        self.map
            .get(id)
            .filter(|e| e.top_k == top_k)
            .map(|e| e.block.clone())
    }

    fn insert(&mut self, id: String, entry: PrefetchEntry) {
        if !self.map.contains_key(&id) {
            self.order.push_back(id.clone());
            if self.order.len() > PREFETCH_CAP {
                if let Some(old) = self.order.pop_front() {
                    self.map.remove(&old);
                }
            }
        }
        self.map.insert(id, entry);
    }
}

/// 挂在 `agent/request` 上的记忆注入监听器。
pub struct MemoryInject {
    source: Arc<dyn MemorySource>,
    cfg: Arc<RwLock<MemoryInjectConfig>>,
    prefetch: Mutex<Prefetch>,
}

impl MemoryInject {
    pub fn new(source: Arc<dyn MemorySource>, cfg: Arc<RwLock<MemoryInjectConfig>>) -> Self {
        Self {
            source,
            cfg,
            prefetch: Mutex::new(Prefetch::default()),
        }
    }

    fn config(&self) -> MemoryInjectConfig {
        self.cfg.read().map(|c| *c).unwrap_or_default()
    }
}

#[async_trait]
impl WaterfallListener for MemoryInject {
    async fn handle(&self, key: &str, payload: &mut Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        let cfg = self.config();
        // 早退只看 `enabled`：旧形状里 top_k 数的是**正文条数**，那个东西随刀① 一起没了，
        // 再留一条 `top_k == 0` 就等于"把条数设 0 会连索引一起关掉"这种说不清的隐藏耦合。
        // top_k 字段本身与"它进预取缓存判据"的形状照旧不动（那是有意的承重，
        // 理由写在 `desktop/src-tauri/src/settings.rs` 的 `memory_inject_config` 头注里）。
        if !cfg.enabled {
            return WaterfallDecision::Continue;
        }
        // 取载荷排在两道配置门之后：它们一个字节都不碰 payload，早退就不必
        // move 进出（守卫的 Drop 也要一次 to_value）。
        let mut guard = match take_request(payload, "MemoryInject") {
            Ok(g) => g,
            Err(d) => return d,
        };
        let req = guard.req();
        // ㉘·A·fix：缓存键并上键位身份——会话中途改归属 = 换键 = 重算；
        // 键位不变则命中同一字节（M1 纪律不破）。scope_id 逐请求查询
        // （desktop = 旁车一次读取，与围栏/审批同成本档）。
        // S3 刀② 再加第三段 `stamp`：注入面那三份输入的 mtime 指纹。它把"会话锁死"
        // 放宽成"文件没动就锁死"——人改 MEMORY.md / 交接正文 = 下一轮生效（方案稿 §C
        // 与 §F 都要这一格）。串不变 ⇒ 字节不变 ⇒ 前缀照旧命中。
        let scope = self.source.scope_id(req.session_id.as_deref());
        let stamp = self.source.index_stamp(req.session_id.as_deref());
        let prefetch_key = req
            .session_id
            .as_deref()
            .map(|id| format!("{id}\u{1}{scope}\u{1}{stamp}"));
        if let Some(key) = &prefetch_key {
            let cached = self
                .prefetch
                .lock()
                .expect("memory prefetch poisoned")
                .get(key, cfg.top_k);
            match cached {
                Some(Some(block)) => {
                    append_block(&mut req.system, Some(&block));
                    return WaterfallDecision::Continue;
                }
                Some(None) => return WaterfallDecision::Continue,
                None => {}
            }
        }
        // 环境块（提示词分层设计 §4.2，v2.139 由 workspace_context 升级）：
        // 归属会话告知项目根 + 日期/平台——即使零记忆也注入（块 = 环境块
        // ∪ 提醒段 ∪ 全局记忆 ∪ 指引）。日期/平台随块同吃会话预取 =
        // 会话锁死（跨天不发提醒，设计稿记档）。
        let mut block = self
            .source
            .context_line(req.session_id.as_deref())
            .map(|line| format!("<environment>\n{line}\n</environment>"));
        // 提醒段（v2.136 项目侧全提醒制）：与工作区行同族，紧随其后；
        // 静态文本宿主给定（正文不注入，主模型按需读文件）。
        if let Some(repo) = self.source.repo_context(req.session_id.as_deref()) {
            let seg = format!("<repo_context>\n{repo}\n</repo_context>");
            append_block(&mut block, Some(&seg));
        }
        // 两段记忆索引（S3 刀① · 方案稿 §C「索引常驻，正文按需」）：全局层与项目层的
        // `MEMORY.md` 指针行合成**一个** `<memory_index>` 块，段头 `[global]` / `[project]`。
        // 内文、钳制、陈旧标注全在宿主侧（与 `<environment>` / `<repo_context>` 同一分工：
        // 块命名归内核、内容真源归宿主）。**两段各自可缺席**，都缺席 ⇒ 整块不生成——
        // 留一对空标签会被读成"有索引但没条目"，与"这两层都没记过"是两个不同结论。
        // 被摘掉的旧形状：`<persistent_memory>`（全局层正文 top-K，还挂着一条按 sqlite
        // 四信号加权的排序档）与独立的 `<project_memory_index>`——设计稿要消灭的正是
        // 这两块各说一套。相关性词表（`terms.rs`）随它的唯一消费者一起离场。
        let mut segs: Vec<String> = Vec::new();
        if let Some(global) = self.source.global_index(req.session_id.as_deref()) {
            segs.push(format!("[global]\n{global}"));
        }
        if let Some(project) = self.source.project_index(req.session_id.as_deref()) {
            segs.push(format!("[project]\n{project}"));
        }
        if !segs.is_empty() {
            let seg = format!("<memory_index>\n{}\n</memory_index>", segs.join("\n"));
            append_block(&mut block, Some(&seg));
        }
        // 主动记忆指引（F3 §4）：随注入块尾——静态文本，与块同吃预取
        // 缓存；无条件注入（见 GUIDANCE_BLOCK 注释的"零记忆也要知道"）。
        append_block(&mut block, Some(GUIDANCE_BLOCK));
        // 首次注入结果入缓存（None 也缓存：无块会话不做每轮空查）
        if let Some(key) = prefetch_key {
            self.prefetch
                .lock()
                .expect("memory prefetch poisoned")
                .insert(
                    key,
                    PrefetchEntry {
                        block: block.clone(),
                        top_k: cfg.top_k,
                    },
                );
        }
        // kernel 每 step 重建 payload，system 恒为 None；仍按"追加"合并，
        // 未来若有其他决策方先写 system 也不覆盖。
        // GUIDANCE_BLOCK 无条件追加过 ⇒ 此处 block 必为 Some，原 `let Some(block)
        // … else 早退` 在新形状下由 append_block 的"add=None 不动"承担。
        append_block(&mut req.system, block.as_deref());
        WaterfallDecision::Continue
    }
}

#[cfg(test)]
mod tests;
