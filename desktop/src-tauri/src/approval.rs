//! 审批中心后端（DESIGN.md §13.2 / §12.1 / 纪律 9）：`ApprovalResolver`
//! 的桌面端实现。
//!
//! Kernel 在执行 `Irreversible` 工具前调用 `resolve`（§7.1 审批门）；
//! 这里把请求作为 `agent://approval-request` 事件推给前端，然后挂起等待
//! `agent_approve` 命令回填决策。请求/等待不进会话日志——日志里的
//! `ApprovalDecided` 由 Kernel 写（纪律 9），本模块只负责"问人"。
//!
//! 并发安全：pending 表是 std Mutex（存取不跨 await）；oneshot 的
//! `rx.await` 在锁外，`agent_approve` 也只碰这张表，不碰会话锁，
//! 与"聊天锁内串行"无死锁交集。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use kernel_agent::{ApprovalDecision, ApprovalResolver, ToolCallReq};
use plugin_api::ToolSpec;
use tokio::sync::oneshot;

use crate::events::{side_effect_str, AgentEvent, ApprovalRequestPayload, EventSink};

struct Pending {
    request: ApprovalRequestPayload,
    tx: oneshot::Sender<ApprovalDecision>,
}

pub struct ApprovalHub {
    sink: Arc<dyn EventSink>,
    pending: Mutex<HashMap<String, Pending>>,
}

impl ApprovalHub {
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        Self {
            sink,
            pending: Mutex::new(HashMap::new()),
        }
    }

    /// `list_pending_approvals`（§13.1）：前端刷新/重连时补拉未决请求。
    pub fn pending(&self) -> Vec<ApprovalRequestPayload> {
        self.pending
            .lock()
            .expect("approval pending poisoned")
            .values()
            .map(|p| p.request.clone())
            .collect()
    }

    /// `agent_approve`（§12.1）：回填决策并唤醒挂起的 step。
    /// 请求不存在（已处理 / 前端陈旧状态）返回 Err。
    pub fn decide(
        &self,
        request_id: &str,
        allowed: bool,
        reason: Option<String>,
    ) -> Result<(), String> {
        let entry = self
            .pending
            .lock()
            .expect("approval pending poisoned")
            .remove(request_id);
        let Some(entry) = entry else {
            return Err(format!("未知或已处理的审批请求：{request_id}"));
        };
        let decision = if allowed {
            ApprovalDecision::allow()
        } else {
            ApprovalDecision::deny(reason.unwrap_or_else(|| "用户拒绝".into()))
        };
        let _ = entry.tx.send(decision);
        Ok(())
    }
}

#[async_trait]
impl ApprovalResolver for ApprovalHub {
    async fn resolve(&self, req: &ToolCallReq, spec: &ToolSpec) -> ApprovalDecision {
        let request = ApprovalRequestPayload {
            request_id: req.id.0.clone(),
            tool: req.tool.clone(),
            args: req.args.clone(),
            side_effect: side_effect_str(&spec.side_effect).to_string(),
        };
        let (tx, rx) = oneshot::channel();
        self.pending
            .lock()
            .expect("approval pending poisoned")
            .insert(
                request.request_id.clone(),
                Pending {
                    request: request.clone(),
                    tx,
                },
            );

        self.sink.emit(&AgentEvent::ApprovalRequest(request));

        // 用户未决策前 step 挂起：审批必须人工介入，不设超时
        // （工具链上的 Timeout 只包执行，不包审批，§7.1）。
        match rx.await {
            Ok(decision) => decision,
            Err(_) => ApprovalDecision::deny("审批通道已关闭（请求被丢弃）"),
        }
    }
}
