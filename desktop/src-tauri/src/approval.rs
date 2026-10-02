//! 审批中心后端（DESIGN.md §13.2 / §12.1 / 纪律 9）：`ApprovalResolver`
//! 的桌面端实现。
//!
//! Kernel 在执行 `Irreversible` 工具前调用 `resolve`（§7.1 审批门）；
//! 这里把请求作为 `agent://approval-request` 事件推给前端，然后挂起等待
//! `agent_approve` 命令回填决策。请求/等待不进会话日志——日志里的
//! `ApprovalDecided` 由 Kernel 写（纪律 9），本模块只负责"问人"。
//!
//! ㉕·对话区①（UI shell doc §3.17，后端为前端服务）两件桌面策略：
//! - **「允许本次对话」豁免集**：`(session_id, tool)` 命中即 `resolve`
//!   直接放行、不再推请求。进程内生命周期 = 会话级，重启后重新问；
//!   Kernel 对每次放行照写 `ApprovalDecided`，转录记录条不缺审计。
//! - **超时自动拒绝**：请求置位即起守夜任务，缺省 100 秒未决策按拒绝
//!   收束（fail-closed）；`decide` 先到先清，守夜抢到移除权才生效。
//!   阈值是否沿用 100s 见 UI shell doc §8.3-5（待用户裁定）。
//!
//! 并发安全：pending/exempt 表是 std Mutex（存取不跨 await）；oneshot 的
//! `rx.await` 在锁外，`agent_approve` 也只碰这两张表，不碰会话锁，
//! 与"聊天锁内串行"无死锁交集。

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use kernel_agent::{ApprovalDecision, ApprovalResolver, ToolCallReq};
use plugin_api::ToolSpec;
use tokio::sync::oneshot;

use crate::events::{side_effect_str, AgentEvent, ApprovalRequestPayload, EventSink};

/// 审批超时缺省（§3.17 照实机：100 秒未响应自动拒绝）。
pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(100);

struct Pending {
    request: ApprovalRequestPayload,
    tx: oneshot::Sender<ApprovalDecision>,
}

pub struct ApprovalHub {
    sink: Arc<dyn EventSink>,
    pending: Arc<Mutex<HashMap<String, Pending>>>,
    exempt: Mutex<HashSet<(String, String)>>,
    timeout: Duration,
}

impl ApprovalHub {
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        Self::with_timeout(sink, APPROVAL_TIMEOUT)
    }

    /// 测试注入短超时用；生产行为同 `new`。
    pub fn with_timeout(sink: Arc<dyn EventSink>, timeout: Duration) -> Self {
        Self {
            sink,
            pending: Arc::new(Mutex::new(HashMap::new())),
            exempt: Mutex::new(HashSet::new()),
            timeout,
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
    /// `scope = Some("session")` 且放行 → 记 (session_id, tool) 豁免，
    /// 本会话后续同工具调用不再问（§3.17「允许本次对话」）。
    /// 请求不存在（已处理 / 前端陈旧状态 / 超时已被守夜拒绝）返回 Err。
    pub fn decide(
        &self,
        request_id: &str,
        allowed: bool,
        reason: Option<String>,
        scope: Option<&str>,
    ) -> Result<(), String> {
        let entry = self
            .pending
            .lock()
            .expect("approval pending poisoned")
            .remove(request_id);
        let Some(entry) = entry else {
            return Err(format!("未知或已处理的审批请求：{request_id}"));
        };
        if allowed && scope == Some("session") {
            let key = (
                entry.request.session_id.clone().unwrap_or_default(),
                entry.request.tool.clone(),
            );
            self.exempt
                .lock()
                .expect("approval exempt poisoned")
                .insert(key);
        }
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
        // 豁免命中：本会话已放行过同工具，直接放行（Kernel 照写
        // ApprovalDecided，投影出记录条——转录里留得住"自动批过"）。
        let exempt_key = (req.session_id.clone().unwrap_or_default(), req.tool.clone());
        if self
            .exempt
            .lock()
            .expect("approval exempt poisoned")
            .contains(&exempt_key)
        {
            return ApprovalDecision::allow();
        }

        let request = ApprovalRequestPayload {
            request_id: req.id.0.clone(),
            tool: req.tool.clone(),
            args: req.args.clone(),
            side_effect: side_effect_str(&spec.side_effect).to_string(),
            // ⑮：子 Agent 派生的调用带 {parent}-sub-{n}，前端打标识徽章
            session_id: req.session_id.clone(),
            // ㉕·对话区①：工具自述进载荷，决策卡理由行的数据源
            description: (!spec.description.is_empty()).then(|| spec.description.clone()),
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

        // 守夜任务（§3.17 超时自动拒绝）：抢到移除权 = 用户未决策，
        // fail-closed 拒绝并唤醒挂起的 step；decide 先移除则此处为空转。
        // 表句柄是 Arc 克隆，任务不借用 self。
        let pending = Arc::clone(&self.pending);
        let request_id = req.id.0.clone();
        let timeout = self.timeout;
        tokio::spawn(async move {
            tokio::time::sleep(timeout).await;
            let entry = pending
                .lock()
                .expect("approval pending poisoned")
                .remove(&request_id);
            if let Some(entry) = entry {
                let _ = entry.tx.send(ApprovalDecision::deny(format!(
                    "超时自动拒绝（{} 秒未响应，fail-closed）",
                    timeout.as_secs()
                )));
            }
        });

        // 用户未决策（或未超时）前 step 挂起：工具链上的 Timeout 只包
        // 执行，不包审批（§7.1）；超时行为由上面的守夜任务承担。
        match rx.await {
            Ok(decision) => decision,
            Err(_) => ApprovalDecision::deny("审批通道已关闭（请求被丢弃）"),
        }
    }
}
