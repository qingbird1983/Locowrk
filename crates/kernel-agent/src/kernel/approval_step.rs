//! 审批 consult 的单点（v2.205 批 Q12·第二刀）。
//!
//! 立这个文件的原因不是行数，是**出处**：等待的两刻（`asked_at` / `decided_at`）是
//! `TurnHeader.active_ms` 的唯一数据源，而它有两个消费点——主循环与 undo 门（`fs_restore`
//! 这类声明 Irreversible 的补偿工具照样过审批门，纪律 9）。两处各写一遍「取时 → await →
//! 落事件 → 被拒闭合」= 同一属性两个出处，undo 门正是最容易漏写第二遍的那一档（取证批
//! v2.203 判出的形状，变异靶 T2 专门钉它）。收成一个函数后，两刻在数据层只有**一处**
//! 被取出、只有**一处**被写进日志。
//!
//! 取时点放在内核而不是决议者：等待就发生在 `self.approve(...).await` 这一次上，宿主审批门
//! 与 headless 都不必为「带出请求时刻」改契约（`plugin-api` 是冻结 ABI，也不该为此动它）。

use plugin_api::ToolSpec;
use serde_json::Value;
use session_store::{Session, SessionEvent};

use super::{Kernel, ToolCallReq};

/// 一次 consult 的决议。构造它的同时，`ApprovalDecided`（含两刻）已落日志，
/// 被拒时 denial 的 `ToolResult` 也已当场闭合（纪律 5：不留悬空调用）。
pub(crate) enum Consulted {
    /// 放行——调用方继续执行工具。
    Allowed,
    /// 拒绝——denial 文案已写进日志，调用方只需处理自己的控制流。
    Denied { message: String },
}

impl Kernel {
    /// 走一次审批 consult 并把决议连同等待的两刻落进会话日志。
    ///
    /// 「没有等待」与「不知道等了多久」在数据层必须可区分：无决议者时 `approve()` 当场
    /// 返回、根本没有请求发生 ⇒ 两刻都写 `None`（投影层据此决定 `active_ms` 给不给，
    /// 不靠猜）。有决议者时两刻齐，且 `decided_at >= asked_at` 由同一把尺保证。
    pub(crate) async fn consult_to_log(
        &self,
        call: &ToolCallReq,
        spec: &ToolSpec,
        session: &mut Session,
    ) -> Consulted {
        let asked_at = self.approval.is_some().then(crate::timeutil::now_ms);
        let decision = self.approve(call, spec).await;
        let decided_at = asked_at.map(|_| crate::timeutil::now_ms());
        session.append(SessionEvent::ApprovalDecided {
            call_id: call.id.clone(),
            allowed: decision.allowed,
            reason: decision.reason.clone(),
            asked_at,
            decided_at,
        });
        if decision.allowed {
            return Consulted::Allowed;
        }
        let message = match decision.reason {
            Some(reason) => format!("denied by approval: {reason}"),
            None => "denied by approval".into(),
        };
        session.append(SessionEvent::ToolResult {
            call_id: call.id.clone(),
            output: Value::Null,
            error: Some(message.clone()),
        });
        Consulted::Denied { message }
    }
}
