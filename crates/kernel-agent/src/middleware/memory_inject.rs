//! Memory-Inject（Context and Memory.md §4.2.3 / §4.3 / §5）：`agent/request`
//! waterfall 监听器——把持久记忆按更新时间取 top-K，注入请求级
//! system prompt（`RequestPayload::system`）；⑬ 起并列注入语义检索的
//! 经历记忆（experiences，"聊过什么、做过什么决策"）。
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
//! 统计与审计日志）。经历记忆（§4.3）检索要过 embedding 模型（异步），
//! 经 [`ExperienceSource`] 独立出 async 契约、组装点可选提供——未提供
//! 或检索失败都跳过经历块，事实记忆照常注入（§4.3 降级策略：语义层
//! 缺席不拖垮注入）。kernel 每 step 从会话重建 payload，`system` 恒从
//! None 起算，天然无重复注入。

use std::sync::{Arc, RwLock};

use async_trait::async_trait;
use cordis_core::{WaterfallDecision, WaterfallListener};
use serde_json::Value;

use session_store::Message;

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
}

/// 记忆检索源契约：宿主实现（desktop 直读 memory.sqlite3，插件侧
/// schema；ALTER 迁移同样归宿主）。同步短查询，不跨 await；任何失败
/// 返回空表（实现内部 warn，不向上抛——注入不值得拦聊天）。
pub trait MemorySource: Send + Sync + 'static {
    /// 按宿主裁决的相关序（desktop = updated_at 新→旧）取至多 limit 条。
    fn recall(&self, limit: usize) -> Vec<MemoryEntry>;
}

/// 单条经历记忆（⑬ 语义检索命中；只带注入需要的字段）。
#[derive(Debug, Clone)]
pub struct ExperienceEntry {
    pub content: String,
    /// 来源会话 id（记忆管理页溯源用；注入文本里只带日期）。
    pub session_id: String,
    /// 创建时间（Unix 秒）。
    pub created_at: i64,
}

/// 经历记忆检索源契约（⑬，Context 文档 §4.3）：宿主实现 = embedding
/// 查询向量 + 经历库余弦检索。异步（embedding 要过网络）；未配置
/// embedding 或检索失败返回空表，实现内部裁决降级。
#[async_trait]
pub trait ExperienceSource: Send + Sync + 'static {
    async fn recall(&self, query: &str, limit: usize) -> Vec<ExperienceEntry>;
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

/// 挂在 `agent/request` 上的记忆注入监听器。
pub struct MemoryInject {
    source: Arc<dyn MemorySource>,
    experiences: Option<Arc<dyn ExperienceSource>>,
    cfg: Arc<RwLock<MemoryInjectConfig>>,
}

impl MemoryInject {
    /// `experiences` = ⑬ 经历检索器（embedding 未配置传 None——只注入
    /// 事实记忆，§4.3 降级）。
    pub fn new(
        source: Arc<dyn MemorySource>,
        experiences: Option<Arc<dyn ExperienceSource>>,
        cfg: Arc<RwLock<MemoryInjectConfig>>,
    ) -> Self {
        Self {
            source,
            experiences,
            cfg,
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
            lines.push_str(&format!(
                "- {} = {}{}\n",
                e.key,
                clip(&e.value),
                tag_suffix(&e.tags)
            ));
        }
        lines.push_str("</persistent_memory>");
        Some(lines)
    }

    /// 经历块文本（⑬，§4.3 语义召回）：`- (日期) 内容` 行。空返回 None。
    fn build_experience_block(entries: &[ExperienceEntry]) -> Option<String> {
        if entries.is_empty() {
            return None;
        }
        let mut lines = String::from(
            "<session_experiences>\n\
             Notes below were semantically recalled from earlier sessions \
             (what was discussed and decided). Apply them when relevant.\n",
        );
        for e in entries {
            let date = date_of(e.created_at);
            lines.push_str(&format!("- ({date}) {}\n", clip(&e.content)));
        }
        lines.push_str("</session_experiences>");
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

/// Unix 秒 → `YYYY-MM-DD`（UTC，注入文本给 LLM 看日期即可）。
fn date_of(secs: i64) -> String {
    // 无 chrono 依赖：civil-from-days（Howard Hinnant 算法）
    let days = secs.div_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02}", if m <= 2 { y + 1 } else { y }, m, d)
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
        let entries = self.source.recall(cfg.top_k);
        let mut block = Self::build_block(&entries);
        // 经历块（⑬）：查询 = 最近一条用户消息；检索失败/未配置 = 跳过，
        // 事实记忆照常注入（§4.3 降级）
        if let Some(src) = &self.experiences {
            let query = req
                .messages
                .iter()
                .rev()
                .find_map(|m| match m {
                    Message::User { content } => Some(content.as_str()),
                    _ => None,
                })
                .unwrap_or_default();
            if !query.is_empty() {
                let found = src.recall(query, cfg.top_k).await;
                block = Self::append_block(block, Self::build_experience_block(&found));
            }
        }
        let Some(block) = block else {
            return WaterfallDecision::Continue;
        };
        // kernel 每 step 重建 payload，system 恒为 None；仍按"追加"合并，
        // 未来若有其他决策方先写 system 也不覆盖
        req.system = Self::append_block(req.system.take(), Some(block));
        // to_value 失败按 fail-open 放行（与 ctx-compact 同纪律）
        match serde_json::to_value(&req) {
            Ok(v) => WaterfallDecision::Replace(v),
            Err(_) => WaterfallDecision::Continue,
        }
    }
}

#[cfg(test)]
mod tests;
