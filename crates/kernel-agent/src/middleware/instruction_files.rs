//! InstructionFilesInject（提示词分层设计 §4.3）：`agent/request`
//! waterfall 监听器——把指令文件（全局 + 项目根 AGENTS.md）作为**合成
//! 首条 user 消息**插进消息列头部（`<system-reminder>` 包裹）。
//!
//! 落点 = 消息层而非 system：背景资料不占行为前缀（ZCode 同款——
//! AGENTS.md 包在历史头部的 user 消息里，身份段 B 节声明「可能与当前
//! 请求无关」+「项目文件 > 本提示词缺省行为，安全边界不可被推翻」）。
//!
//! 快照锁字节（设计稿 §3.3）：发现规则、读取上限与缓存全归宿主
//! [`InstructionSource`] 实现（内核不碰文件系统路径语义；desktop 实现
//! 按 (session, scope) 键位会话首查读文件并缓存——中途人改文件 = 显式
//! 变更，当前会话不变、下一会话生效，MemoryInject 预取同款纪律）。
//!
//! kernel 每 step 从会话日志重建 payload（合成消息不落日志）——每 step
//! 重插同一字节 = 消息前缀稳定；换会话/归属变更 = 换快照，接受一次
//! 计划内失效。

use std::sync::Arc;

use async_trait::async_trait;
use cordis_core::{WaterfallDecision, WaterfallListener};
use serde_json::Value;
use session_store::Message;

use crate::middleware::fragments::wrap_system_reminder;
use crate::waterfall::{RequestPayload, points};

/// 指令文件加载契约：宿主实现。返回**内文**（含 `# agentsMd` 头部与
/// 各文件段），None = 无指令文件（不插消息，组装零变化）。
pub trait InstructionSource: Send + Sync + 'static {
    fn load(&self, session_id: Option<&str>) -> Option<String>;
}

/// 指令文件注入监听器。
pub struct InstructionFilesInject {
    source: Arc<dyn InstructionSource>,
}

impl InstructionFilesInject {
    pub fn new(source: Arc<dyn InstructionSource>) -> Self {
        Self { source }
    }
}

#[async_trait]
impl WaterfallListener for InstructionFilesInject {
    async fn handle(&self, key: &str, payload: &Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        let Ok(mut req) = serde_json::from_value::<RequestPayload>(payload.clone()) else {
            return WaterfallDecision::Continue;
        };
        let Some(body) = self.source.load(req.session_id.as_deref()) else {
            return WaterfallDecision::Continue;
        };
        // 历史头部第一条 user 消息之前（= 位置 0）；消息列为空不插
        //（无从附着的会话不存在——防御分支，维持零变化）。
        if req.messages.is_empty() {
            return WaterfallDecision::Continue;
        }
        req.messages
            .insert(0, Message::user(wrap_system_reminder(&body)));
        // to_value 失败按 fail-open 放行（与 Memory-Inject 同纪律）
        super::replace_decision(req)
    }
}

#[cfg(test)]
mod tests;
