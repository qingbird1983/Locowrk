//! 副作用补偿执行器（DESIGN.md §6.3）：session-store 产出**纯计划**
//! （`compensation::plan`），内核执行——逆序调 undo_tool、Irreversible 记
//! `ManualIntervention`。session-store 不依赖 ToolSource，执行只能在内核。
//!
//! undo 调用契约（FFI 不传 CallCtx，工具靠参数自寻数据）：
//! `{"call_id": <原调用 id>, "args": <原调用 args>, "output": <原调用成功结果>}`。
//! 补偿工具必须幂等、以原 `call_id` 为去重键（纪律 7 / 纪律 10）：
//! - 幂等闸门 = `CompensatedLedger`（undo **成功后**登记；失败的补偿可重试）
//! - undo 调用本身按纪律 5 记日志，用**新的** call_id（`{原id}-undo`）——
//!   复用原 id 会让会话投影出现两条同 id 的 ToolResult，破坏后续真实模型
//!   协议（OpenAI 风格 tool_call_id 必须唯一）
//! - undo 工具若声明 Irreversible（如 fs_restore），照走审批门（纪律 9）

use std::sync::Arc;

use plugin_api::{CallCtx, CallId};
use serde_json::{Value, json};
use session_store::{CompensatedLedger, Compensation, Session, SessionEvent, plan};

use crate::kernel::{Consulted, Kernel, KernelError, ToolCallReq, ToolSource};

/// 一次补偿执行的结报。
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CompensationReport {
    /// 已完成补偿（含账本命中直接跳过的）。
    pub undone: Vec<CallId>,
    /// 不可逆，只能人工介入（已记 ManualIntervention 事件）。
    pub manual: Vec<(CallId, String)>,
    /// 补偿失败（未登记账本，可再次执行重试；已按纪律 5 以失败 ToolResult 闭合）。
    pub failed: Vec<(CallId, String)>,
}

impl Kernel {
    /// 对 `after_seq` 之后成功执行过的 Compensable 调用做逆序补偿（§6.3）。
    /// 由会话回退的编排层调用：先补偿副作用，再移动游标（§6.4 两层互补）。
    pub async fn compensate_after(
        &self,
        session: &mut Session,
        ledger: &mut CompensatedLedger,
        after_seq: usize,
    ) -> Result<CompensationReport, KernelError> {
        let actions = plan(session.events(), after_seq);
        let mut report = CompensationReport::default();
        if actions.is_empty() {
            return Ok(report);
        }
        let source = self.scoped_dyn::<dyn ToolSource>()?;

        for action in actions {
            match action {
                Compensation::ManualIntervention { call_id, reason } => {
                    session.append(SessionEvent::ManualIntervention {
                        call_id: call_id.clone(),
                        reason: reason.clone(),
                    });
                    report.manual.push((call_id, reason));
                }
                Compensation::Undo { call_id, undo_tool } => {
                    if ledger.contains(&call_id) {
                        // 幂等闸门：这个调用已经补偿过（跨重试去重，纪律 10）
                        report.undone.push(call_id);
                        continue;
                    }
                    self.run_undo(session, &source, ledger, &mut report, call_id, undo_tool)
                        .await;
                }
            }
        }
        Ok(report)
    }

    /// 执行单条 Undo：回填原 args/output → 记 ToolCall → （审批）→ 执行 →
    /// 记 ToolResult → 成功才登记账本。
    async fn run_undo(
        &self,
        session: &mut Session,
        source: &Arc<dyn ToolSource>,
        ledger: &mut CompensatedLedger,
        report: &mut CompensationReport,
        call_id: CallId,
        undo_tool: String,
    ) {
        let Some((orig_args, orig_output)) = find_call(session.events(), &call_id) else {
            report
                .failed
                .push((call_id, "original call not found in session log".into()));
            return;
        };
        // 恢复操作不收会话模式约束（㉘·B）：补偿是 recovery 语义，带会话
        // 身份会让计划模式挡住 undo——恒 None（绕过目录级会话过滤）
        let tool = match source.resolve(None, &undo_tool).await {
            Ok(tool) => tool,
            Err(e) => {
                report
                    .failed
                    .push((call_id, format!("undo tool `{undo_tool}` unavailable: {e}")));
                return;
            }
        };

        let undo_id = CallId(format!("{}-undo", call_id.0));
        let undo_args = json!({
            "call_id": call_id.0,
            "args": orig_args,
            "output": orig_output,
        });
        // 先写日志再执行（纪律 5）：补偿调用与普通调用同纪律
        session.append(SessionEvent::ToolCall {
            call_id: undo_id.clone(),
            tool: undo_tool.clone(),
            args: undo_args.clone(),
            plugin_id: tool.plugin_id().map(str::to_owned),
            plugin_version: tool.plugin_version().clone(),
            side_effect: tool.spec().side_effect.clone(),
        });

        // undo 工具声明 Irreversible 时照走审批门（纪律 9；fs_restore 即此类）
        if matches!(
            tool.spec().side_effect,
            plugin_api::SideEffect::Irreversible
        ) {
            let req = ToolCallReq {
                id: undo_id.clone(),
                tool: undo_tool.clone(),
                args: undo_args.clone(),
                session_id: None,
            };
            // 与主循环同一条 consult 通路（`kernel/approval_step.rs`，v2.205）：决议、
            // 等待的两刻、被拒时的 denial 闭合都在那个单点里做完——undo 门不另发明
            // 一套时刻口径（取证批 v2.203 判出的形状，变异靶 T2 钉的就是这里）。
            if let Consulted::Denied { message } =
                self.consult_to_log(&req, tool.spec(), session).await
            {
                report.failed.push((call_id, message));
                return;
            }
        }

        let mut cctx = CallCtx::new();
        cctx.call_id = Some(undo_id.clone());
        // 补偿调用**故意不进** `active_calls` 登记表（specs/04 · C1 的一侧）：
        // 它没有 session_id（上面两行只填 call_id），而登记表只收带会话身份的调用；
        // 就算能收也不该收——按停止去取消一条正在回滚的写操作，得到的正是纪律 1
        // 要避免的"半途状态"（specs/04 风险①）。取消信号到不了回滚这条路是有意的，
        // 不是漏接。
        // 复用主循环同一条中间件链：审计 / 超时 / 熔断对补偿调用同样生效
        let out = crate::chain::ToolChain::new(self.mw.as_slice(), Arc::clone(&tool))
            .call(&mut cctx, undo_args)
            .await;

        match out {
            Ok(output) => {
                session.append(SessionEvent::ToolResult {
                    call_id: undo_id,
                    output,
                    error: None,
                });
                ledger.claim(&call_id);
                report.undone.push(call_id);
            }
            Err(e) => {
                let msg = e.to_string();
                session.append(SessionEvent::ToolResult {
                    call_id: undo_id,
                    output: Value::Null,
                    error: Some(msg.clone()),
                });
                report.failed.push((call_id, msg));
            }
        }
    }
}

/// 回填原调用的 args 与成功 output（§4.2：ToolResult 事件只带 call_id，
/// 内容要从 ToolCall / ToolResult 两个事件对拼出来）。
fn find_call(events: &[SessionEvent], call_id: &CallId) -> Option<(Value, Value)> {
    let mut args = None;
    let mut output = None;
    for e in events {
        match e {
            SessionEvent::ToolCall {
                call_id: c,
                args: a,
                ..
            } if c == call_id => args = Some(a.clone()),
            SessionEvent::ToolResult {
                call_id: c,
                output: o,
                error: None,
                ..
            } if c == call_id => output = Some(o.clone()),
            _ => {}
        }
    }
    Some((args?, output?))
}

#[cfg(test)]
mod tests;
