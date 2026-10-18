//! Memory-Inject（Context and Memory.md §4.2.3 / §5）：`agent/request`
//! waterfall 监听器——把**全局层**持久记忆按更新时间取 top-K，注入
//! 请求级 system prompt（`RequestPayload::system`）。
//!
//! **注入面（v2.136 裁定「项目侧全提醒制」）**：流水线只自动携带全局
//! 记忆（用户层面的喜好/全局要求，保持简短）——项目层事实与项目记忆
//! 文件（`<repo>/.Locowrk/`）不再整段注入，改为 [`MemorySource::repo_context`]
//! 提供的**静态提醒段**（记忆目录里有什么、在什么节点读/更新），由
//! 当前工作的主模型按需用文件工具查看与维护；结构化项目事实走
//! memory_read / memory_search 按需查。
//!
//! Q2（MR 稿 §4.1）在同一个块里追加一节 [`MemorySource::project_index`]＝
//! **项目记忆的指针索引**（一行一条 `- key — hook`）。它**不是**对上述裁定的
//! 反转：裁定禁的是"项目事实全文灌进 system"，索引给的是"本项目记了哪些条"，
//! 正文一个字都不进——这正是 MR 稿 §0 第三条根因（注入面三重盲：项目层有货
//! 而模型不知道有货）的对症件。
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
//! 时检索一次并缓存块文本**，后续轮复用同一字节；`memory_write` 中途
//! 写入不再改写本会话前缀。top_k 配置变更 = 显式变更，重算接受失效。
//! 缓存键 = `(session_id, scope_id)`（㉘·A·fix）：键位不变逐字节复用，
//! 会话改归属自动重算。无会话身份 = 每轮检索（一次性进程等价会话，
//! headless 语义不变）。

use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use cordis_core::{WaterfallDecision, WaterfallListener};
use serde_json::Value;

use crate::waterfall::{append_block, points, take_request};

mod contract;
mod terms;

pub use contract::{LearnWeights, MemoryEntry, MemoryInjectConfig, MemorySource, RecallQuery};
use terms::relevance_terms;

/// 单条 value 注入上限（字符）：记忆库单条可达 256KiB，原样注入一轮就
/// 顶爆窗口；超长截断并标注（全文在记忆管理页可看）。
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

    /// 记忆块文本（给 LLM 的英文说明 + `key = value [tags: …]` 行）。
    /// 空条目表返回 None（无可注入）。
    fn build_block(entries: &[MemoryEntry]) -> Option<String> {
        if entries.is_empty() {
            return None;
        }
        let mut lines = String::from(
            "<persistent_memory>\n\
             Facts below were saved to the user's persistent memory in earlier \
             sessions (key = value). They persist across sessions; apply them \
             when relevant to this conversation.\n",
        );
        for e in entries {
            let tier = if e.scope.is_empty() {
                String::new()
            } else {
                format!("[{}] ", e.scope)
            };
            lines.push_str(&format!(
                "- {tier}{} = {}{}\n",
                e.key,
                clip(&e.value),
                tag_suffix(&e.tags)
            ));
        }
        lines.push_str("</persistent_memory>");
        Some(lines)
    }
}

/// 单条注入上限（字符）：与事实记忆同口径——超长截断并标注，全文在
/// 记忆管理页可看。
fn clip(value: &str) -> String {
    let char_count = value.chars().count();
    if char_count > VALUE_INJECT_MAX_CHARS {
        let mut s: String = value.chars().take(VALUE_INJECT_MAX_CHARS).collect();
        s.push_str(" …(truncated)");
        s
    } else {
        value.to_string()
    }
}

fn tag_suffix(tags: &str) -> String {
    if tags.is_empty() {
        String::new()
    } else {
        format!(" [tags: {tags}]")
    }
}

#[async_trait]
impl WaterfallListener for MemoryInject {
    async fn handle(&self, key: &str, payload: &mut Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        let cfg = self.config();
        if !cfg.enabled || cfg.top_k == 0 {
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
        let scope = self.source.scope_id(req.session_id.as_deref());
        let prefetch_key = req
            .session_id
            .as_deref()
            .map(|id| format!("{id}\u{1}{scope}"));
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
        // 全局记忆注入（v2.136 裁定）：只取全局层（用户层面的喜好/全局
        // 要求）；项目层事实不自动注入，由模型 memory_read / memory_search
        // 按需查（见 GUIDANCE_BLOCK）。排序真源在宿主：队列 Q9（ML §3.3/§3.4）
        // 起为**四信号现算加权**，`learn_enabled=false` 时逐字节回到现役的
        // `updated_at DESC, key`——内核只负责把相关性词表递下去（取数时刻 =
        // 会话首查，被下面的预取缓存包住 = 一个会话一次）。
        let query = RecallQuery {
            terms: if cfg.learn.enabled {
                relevance_terms(&req.messages)
            } else {
                Vec::new()
            },
            weights: cfg.learn,
        };
        let entries = self
            .source
            .recall_relevant(req.session_id.as_deref(), cfg.top_k, &query);
        let mem = Self::build_block(&entries);
        append_block(&mut block, mem.as_deref());
        // 项目记忆索引（Q2 · MR §4.1）：紧随全局层之后——指针行，正文不进。
        // 宿主给定内文，标签在这里包（与 `<repo_context>` / `<environment>` 同形，
        // 块命名由内核管、内容真源在宿主）。整节缺席 = 未归属 / 项目层空。
        if let Some(index) = self.source.project_index(req.session_id.as_deref()) {
            let seg = format!("<project_memory_index>\n{index}\n</project_memory_index>");
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
