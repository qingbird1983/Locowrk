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
//! 链位（§3.6）：ToolExposure（⑫）→ **Memory-Inject** → ctx-compact——
//! 记忆是"原料"，压缩是"分配预算"，注入必须发生在压缩估算之前（注册
//! 次序即执行次序，组装点负责先注册本件再注册 ctx-compact）。
//!
//! 系统提示永不压缩（§5.3 冲突 2）由结构保证：记忆写在消息列表之外的
//! `system` 字段里，ctx-compact 只改写 `messages`，会话日志也不落注入
//! 内容——投影零改动。
//!
//! 与 mw-audit / ctx-compact 同理落在宿主侧：事实记忆库归 memory-sqlite
//! 插件所有，内核不碰 SQL——检索经 [`MemorySource`] trait（组装点提供
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

use crate::waterfall::{RequestPayload, points};

/// 单条记忆（宿主检索实现返回；只带注入需要的字段）。
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub key: String,
    pub value: String,
    /// 短标签（memory-sqlite 的 tags 为单串，空串 = 无标签）。
    pub tags: String,
    /// 更新时间（Unix 秒）；当前仅随条目透出，排序由宿主实现裁决。
    pub updated_at: i64,
    /// 层标（㉘·A·fix 双层记忆，Context 文档 §4.6）：`"global"`（用户偏好，
    /// 全项目可见）| `"project"`（本项目记录）；空串 = 不标层（旧行为）。
    pub scope: String,
}

/// 记忆检索源契约：宿主实现（desktop 直读 memory.sqlite3，插件侧
/// schema；ALTER 迁移同样归宿主）。同步短查询，不跨 await；任何失败
/// 返回空表（实现内部 warn，不向上抛——注入不值得拦聊天）。
///
/// `session_id`（㉘）：本 step 的会话 id——宿主据此把召回限定在该会话
/// 归属的项目内（项目级记忆隔离，Context 文档 §4.6）；内核不解释
/// workspace 概念，只透传 id。`None` = 无会话上下文（如测试装配）。
pub trait MemorySource: Send + Sync + 'static {
    /// 按宿主裁决的相关序（desktop = updated_at 新→旧）取至多 limit 条。
    fn recall(&self, session_id: Option<&str>, limit: usize) -> Vec<MemoryEntry>;

    /// 会话的键位身份（㉘·A·fix）：预取缓存键 = `(session_id, scope_id)`。
    /// 会话中途改归属 = 换键 = 首查重算（键位不变则逐字节复用，M1 纪律
    /// 不破）。缺省空串 = 不参与键（旧行为，装配测试零负担）。
    fn scope_id(&self, session_id: Option<&str>) -> String {
        let _ = session_id;
        String::new()
    }

    /// 注入块头部的工作区上下文行（㉘·A·fix）：归属会话告知模型"当前项目
    /// 在哪"，让它不再靠旧记忆猜路径。逐会话稳定、同吃预取缓存。缺省
    /// `None` = 不带行（未归属/headless）。
    fn context_line(&self, session_id: Option<&str>) -> Option<String> {
        let _ = session_id;
        None
    }

    /// 仓库交接/提醒段（v2.136 项目侧全提醒制）：归属会话的
    /// `<repo>/.Locowrk/` **静态提醒文本**——目录里有什么、在什么节点
    /// 读/更新，正文不再注入，由主模型按需用文件工具查看与维护。宿主
    /// 实现；缺省 `None` = 无提醒段（headless / 未归属 / 开关关），组装
    /// 零变化。文本静态 = 跨轮逐字节稳定（M1 纪律不破）。
    fn repo_context(&self, session_id: Option<&str>) -> Option<String> {
        let _ = session_id;
        None
    }
}

/// 注入参数（settings.json `memory` 区；Context 文档 §6.2 的
/// max_inject_count，缺省 5——约束清单 §3"注入条数可配"）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryInjectConfig {
    pub enabled: bool,
    pub top_k: usize,
}

impl Default for MemoryInjectConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            top_k: 5,
        }
    }
}

/// 单条 value 注入上限（字符）：记忆库单条可达 256KiB，原样注入一轮就
/// 顶爆窗口；超长截断并标注（全文在记忆管理页可看）。
const VALUE_INJECT_MAX_CHARS: usize = 500;

/// 预取缓存上限（FIFO 驱逐最旧会话）：与 ToolExposure 钉死表同款保险丝。
const PREFETCH_CAP: usize = 128;

/// 主动记忆指引（F3 起随注入块尾；v2.136 按新记忆模型改写）。**无条件
/// 注入**（中间件启用即带）——主动记忆的本意就是零记忆的新会话也要知道
/// "该自己记"，指引缺席 = 主动原则落空；静态文本与块同吃预取缓存（M1
/// 纪律不破）。memory 注入关（`memory.enabled=false` / top_k=0）= 整块
/// 含指引一起缺席。分工（v2.136 裁定）：全局记忆走 memory_write KV；
/// 项目记忆（.Locowrk/ 文件）由当前工作的主模型自己读/写/改——软件不
/// 代劳，只在 [`MemorySource::repo_context`] 提醒段给出节点钩子。
const GUIDANCE_BLOCK: &str = "<memory_guidance>\n\
     Proactive memory: you are expected to maintain memory yourself — do not \
     wait to be asked.\n\
     - When the user states a lasting preference or standing instruction \
     (\"always ...\", \"from now on ...\"), save it immediately with \
     memory_write using scope=\"global\"; prefer updating an existing key \
     over creating near-duplicates.\n\
     - Durable project facts worth structured recall: save them with \
     memory_write (project scope); look them up later with memory_read / \
     memory_search when needed.\n\
     - Project working memory (requirements.md / handoff.md / notes/ under \
     the project's .Locowrk/ directory) is yours to maintain directly with \
     file tools — the repo context reminder below tells you when to read \
     and update them.\n\
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

    /// system 现值追加注入块（现值为 None = 原样；块为 None = 不动）。
    fn append_block(system: Option<String>, block: Option<String>) -> Option<String> {
        match (system, block) {
            (Some(prev), Some(b)) => Some(format!("{prev}\n\n{b}")),
            (None, Some(b)) => Some(b),
            (prev, None) => prev,
        }
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
    async fn handle(&self, key: &str, payload: &Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        let cfg = self.config();
        if !cfg.enabled || cfg.top_k == 0 {
            return WaterfallDecision::Continue;
        }
        let Ok(mut req) = serde_json::from_value::<RequestPayload>(payload.clone()) else {
            return WaterfallDecision::Continue;
        };
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
                    req.system = Self::append_block(req.system.take(), Some(block));
                    return super::replace_decision(req);
                }
                Some(None) => return WaterfallDecision::Continue,
                None => {}
            }
        }
        // 工作区上下文行（㉘·A·fix）：归属会话先告知"当前项目在哪"——
        // 即使零记忆也注入（块 = 上下文行 ∪ 提醒段 ∪ 全局记忆 ∪ 指引）。
        let mut block = self
            .source
            .context_line(req.session_id.as_deref())
            .map(|line| format!("<workspace_context>\n{line}\n</workspace_context>"));
        // 提醒段（v2.136 项目侧全提醒制）：与工作区行同族，紧随其后；
        // 静态文本宿主给定（正文不注入，主模型按需读文件）。
        if let Some(repo) = self.source.repo_context(req.session_id.as_deref()) {
            block = Self::append_block(
                block,
                Some(format!("<repo_context>\n{repo}\n</repo_context>")),
            );
        }
        // 全局记忆注入（v2.136 裁定）：只取全局层（用户层面的喜好/全局
        // 要求），按 updated_at 新→旧取 top_k；项目层事实不自动注入，
        // 由模型 memory_read / memory_search 按需查（见 GUIDANCE_BLOCK）。
        let entries = self.source.recall(req.session_id.as_deref(), cfg.top_k);
        block = Self::append_block(block, Self::build_block(&entries));
        // 主动记忆指引（F3 §4）：随注入块尾——静态文本，与块同吃预取
        // 缓存；无条件注入（见 GUIDANCE_BLOCK 注释的"零记忆也要知道"）。
        block = Self::append_block(block, Some(GUIDANCE_BLOCK.to_string()));
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
        let Some(block) = block else {
            return WaterfallDecision::Continue;
        };
        // kernel 每 step 重建 payload，system 恒为 None；仍按"追加"合并，
        // 未来若有其他决策方先写 system 也不覆盖
        req.system = Self::append_block(req.system.take(), Some(block));
        // to_value 失败按 fail-open 放行（与 ctx-compact 同纪律）
        super::replace_decision(req)
    }
}

#[cfg(test)]
mod tests;
