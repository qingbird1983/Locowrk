//! Memory-Inject（Context and Memory.md §4.2.3 / §5）：`agent/request`
//! waterfall 监听器——把持久记忆按更新时间取 top-K，注入请求级
//! system prompt（`RequestPayload::system`）。
//!
//! 链位（§3.6）：ToolExposure（⑫）→ **Memory-Inject** → ctx-compact——
//! 记忆是"原料"，压缩是"分配预算"，注入必须发生在压缩估算之前（注册
//! 次序即执行次序，组装点负责先注册本件再注册 ctx-compact）。
//!
//! 系统提示永不压缩（§5.3 冲突 2）由结构保证：记忆写在消息列表之外的
//! `system` 字段里，ctx-compact 只改写 `messages`，会话日志也不落注入
//! 内容——投影零改动。
//!
//! 与 mw-audit / ctx-compact 同理落在宿主侧：记忆库归 memory-sqlite
//! 插件所有，内核不碰 SQL——检索经 [`MemorySource`] trait（组装点提供
//! 实现，与 ApprovalResolver 同款"内核定契约、组装点实现"模式；不经
//! ToolSource 走工具管道，否则每 step 一次 memory_list 会污染 ⑩ 的使用
//! 统计与审计日志）。检索失败 = 空表降级（fail-open，注入不拦聊天）。
//! kernel 每 step 从会话重建 payload，`system` 恒从 None 起算，天然无
//! 重复注入。

use std::sync::{Arc, RwLock};

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
}

/// 记忆检索源契约：宿主实现（desktop 直读 memory.sqlite3，插件侧
/// schema；ALTER 迁移同样归宿主）。同步短查询，不跨 await；任何失败
/// 返回空表（实现内部 warn，不向上抛——注入不值得拦聊天）。
pub trait MemorySource: Send + Sync + 'static {
    /// 按宿主裁决的相关序（desktop = updated_at 新→旧）取至多 limit 条。
    fn recall(&self, limit: usize) -> Vec<MemoryEntry>;
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
    cfg: Arc<RwLock<MemoryInjectConfig>>,
}

impl MemoryInject {
    pub fn new(source: Arc<dyn MemorySource>, cfg: Arc<RwLock<MemoryInjectConfig>>) -> Self {
        Self { source, cfg }
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
            let char_count = e.value.chars().count();
            let (value, suffix) = if char_count > VALUE_INJECT_MAX_CHARS {
                (
                    e.value
                        .chars()
                        .take(VALUE_INJECT_MAX_CHARS)
                        .collect::<String>(),
                    " …(truncated)",
                )
            } else {
                (e.value.clone(), "")
            };
            let tags = if e.tags.is_empty() {
                String::new()
            } else {
                format!(" [tags: {}]", e.tags)
            };
            lines.push_str(&format!("- {} = {value}{suffix}{tags}\n", e.key));
        }
        lines.push_str("</persistent_memory>");
        Some(lines)
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
        let entries = self.source.recall(cfg.top_k);
        let Some(block) = Self::build_block(&entries) else {
            return WaterfallDecision::Continue;
        };
        // kernel 每 step 重建 payload，system 恒为 None；仍按"追加"合并，
        // 未来若有其他决策方先写 system 也不覆盖
        req.system = Some(match req.system {
            Some(prev) => format!("{prev}\n\n{block}"),
            None => block,
        });
        // to_value 失败按 fail-open 放行（与 ctx-compact 同纪律）
        match serde_json::to_value(&req) {
            Ok(v) => WaterfallDecision::Replace(v),
            Err(_) => WaterfallDecision::Continue,
        }
    }
}

#[cfg(test)]
mod tests;
