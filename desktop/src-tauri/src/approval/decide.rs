//! 决策表那半（DESIGN §7.2 / §12.1）：`ApprovalResolver::resolve` 的桌面端实现。
//!
//! 从 `approval/mod.rs` 拆出来的理由与 `fence.rs` 同源：**七序决策表是"读规则、给裁决"，
//! 而 mod.rs 那一半是"发请求、挂起等待、守夜超时"**——两半唯一的耦合点是 `self` 上那
//! 四张表（`modes` / `rules` / `exempt` / `pending`）与 `timeout_*`。混在一个文件里时
//! 470 行、两种变化理由（决策序变了 / 等待与超时机制变了），改哪一边都得整个读一遍。
//!
//! 拆分口径：实现**一字未改**（次序、reason 文案、读盘次数、注释里的 M-18a/M-18b 记档
//! 全数原样搬过来）。T1 是纯结构收敛，任何行为差异都算拆错了。
//!
//! 为什么测试不用改：trait 实现不产生调用点，`crate::approval::ApprovalHub` 的公开面
//! 没动；`Pending` / `command_subject` / `root_session_id` 这些本模块私有件靠
//! "子模块看得见祖先模块的私有项"这条 Rust 可见性规则直接引用，不额外放宽到 `pub`。

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use kernel_agent::{ApprovalDecision, ApprovalResolver, ToolCallReq};
use plugin_api::ToolSpec;
use tokio::sync::oneshot;

use crate::events::{side_effect_str, AgentEvent, ApprovalRequestPayload};

use super::{
    command_subject, command_touches_outside, root_session_id, ApprovalHub, Pending,
    TIMEOUT_REASON_PREFIX,
};

#[async_trait]
impl ApprovalResolver for ApprovalHub {
    async fn resolve(&self, req: &ToolCallReq, spec: &ToolSpec) -> ApprovalDecision {
        // 子会话改道（v2.145）：{parent}-sub-{n} → parent——权限档继承、
        // 豁免/规则键位、越界门围栏、卡片工作区全部取父根会话。
        let root_sid = root_session_id(req.session_id.as_deref());
        // ㉘·C 决策表（DESIGN §7.2，序即优先级）：①计划档硬拒 ②硬门（隐私
        // + 越界）③项目拒绝规则 ④模式放行（一等公民）⑤项目允许规则
        // ⑥会话豁免 ⑦决策卡。modes 缺席 = 退化为⑥⑦（旧行为；kernel-bin
        // 桥不装配决策表）。
        let mode = self.modes.as_ref().map(|m| m.mode_of(root_sid.as_deref()));
        if mode == Some(crate::permission_mode::PermissionMode::Plan)
            && !matches!(spec.side_effect, plugin_api::SideEffect::Pure)
        {
            // 正常路径已被目录级围栏挡住（非 Pure 不出目录），此处纵深防御
            return ApprovalDecision::deny("计划模式只读：不可写/不可逆调用一律拒绝");
        }

        // M-18a：**一次决策只解析一次会话上下文**。生产 resolver
        // （`workspace.rs::session_context_resolver`）每次调用都读一遍会话
        // 旁车 `.meta.json`，再对 `[数据根] ∪ settings.fsRoots` 逐个
        // `canonicalize`（系统调用）；改前 `resolve` 有四个独立解析点（越界门
        // `scan_roots` + ③deny/⑤allow/卡载荷各一次 `workspace_of`），最坏
        // 路径把同一份旁车读四遍（实测真值见
        // `tests_approval/ctx_parse.rs`）。放在①之后 = Plan 档纵深防御直接
        // 拒绝时不付这笔开销。生产装配三件齐（`kernel_stack.rs`）⇒ 本行
        // 只减少解析、不新增；只有 `modes`/`rules` 都缺席的最小组装（旧测试）
        // 会多一次，记档取舍。
        //
        // 顺带的一致性收益：③⑤与卡载荷现在看**同一份快照**，改前四次解析
        // 之间旁车若被改写（关联工作区变更）会让同一次决策内部不自洽。
        let ctx = root_sid
            .as_deref()
            .and_then(|sid| self.ctx_of.as_ref().map(|f| f(sid)));
        let workspace = ctx.as_ref().and_then(|c| c.workspace.clone());

        // ② 硬门：隐私门（privacy.rs 清单，任何档位都拦——完全访问也不例外，
        // 用户裁定"最危险的触及用户隐私的或本地隐私的有拦截"）+ 越界门
        // （㉘·B 外部路径门；围栏改全局声明根后 = 越出声明根才触发）。
        // 命中即强制弹卡且只许一次（不进豁免集、模式放行被跳过）。
        let sensitive = match &self.modes {
            Some(_) => crate::privacy::check(&req.tool, &req.args),
            None => None,
        };
        let external = match &self.modes {
            Some(_) => command_touches_outside(&req.tool, &req.args, ctx.as_ref()),
            None => false,
        };
        if sensitive.is_none() && !external {
            let subject = command_subject(&req.tool, &req.args);
            // ③ + ⑤ 项目规则**一次读盘两判定**（M-18b）：两级同出一个文件
            // （`permissions/<hash>.json` 的 deny/allow 两数组），改前各读
            // 一遍 = 同一份 KB 级 JSON 解析两次、且两次之间被人改就会让
            // ③⑤看到两个版本。这里一次判完存进 `verdict`，③用 `denied`、
            // ⑤用 `allowed`——**决策表次序一字未改**（④模式放行仍夹在中间，
            // 所以 `allowed` 命中也可能被④提前放行，reason 不同）。
            let verdict = match (&self.rules, workspace.as_deref()) {
                (Some(rules), Some(ws)) => Some(rules.verdicts(ws, &req.tool, subject)),
                _ => None,
            };
            // ③ 项目拒绝规则（v2.145）：UI 不产 deny，手改 JSON 生效；
            // 先于模式放行 = 用户明示拒绝连 Auto 也拦（比 ZCode 的 yolo
            // 先于 deny 更安全，设计稿 §2.1 有意偏离记档）。
            if verdict.is_some_and(|v| v.denied) {
                return ApprovalDecision::deny(
                    "项目权限规则拒绝（deny 清单命中）：编辑或删除数据目录 \
                     permissions/ 下的规则文件以解除",
                );
            }
            // ④ 模式放行（一等公民，㉘·C；审计 reason 带 mode: 前缀，前端
            // 据此不渲染记录条）。只读直通 = 所有档位（隐私门已在上面临过）。
            use crate::permission_mode::PermissionMode as PM;
            if matches!(spec.side_effect, plugin_api::SideEffect::Pure) {
                return ApprovalDecision {
                    allowed: true,
                    reason: Some(if mode == Some(PM::Plan) {
                        "mode:plan-readonly".into()
                    } else {
                        "mode:pure".into()
                    }),
                };
            }
            match mode {
                Some(PM::Auto) => {
                    return ApprovalDecision {
                        allowed: true,
                        reason: Some("mode:auto".into()),
                    };
                }
                Some(PM::AcceptEdits)
                    if matches!(spec.side_effect, plugin_api::SideEffect::Compensable { .. }) =>
                {
                    return ApprovalDecision {
                        allowed: true,
                        reason: Some("mode:acceptEdits".into()),
                    };
                }
                _ => {}
            }
            // ⑤ 项目允许规则（v2.145 粘性授权）：工具级或精确命令命中即
            // 放行（reason 不带 mode: 前缀 = 前端照常渲染记录条，审计可见）。
            // 判定结果来自上面 ③ 那次读盘的同一份快照（M-18b）。
            if verdict.is_some_and(|v| v.allowed) {
                return ApprovalDecision {
                    allowed: true,
                    reason: Some("rule:project-allow".into()),
                };
            }
            // ⑥ 会话豁免：本会话（含子代理，键 = 父根会话）已放行过同工具，
            // 直接放行（Kernel 照写 ApprovalDecided，投影出记录条——转录里
            // 留得住"自动批过"）
            let exempt_key = (root_sid.clone().unwrap_or_default(), req.tool.clone());
            if self
                .exempt
                .lock()
                .expect("approval exempt poisoned")
                .contains(&exempt_key)
            {
                return ApprovalDecision::allow();
            }
        }

        // 卡载荷的环境行直接用 M-18a 那次解析的结果（改前这里再解析一遍）。
        let request = ApprovalRequestPayload {
            request_id: req.id.0.clone(),
            tool: req.tool.clone(),
            args: req.args.clone(),
            side_effect: side_effect_str(&spec.side_effect).to_string(),
            // ⑮：子 Agent 派生的调用带 {parent}-sub-{n}，前端打标识徽章
            session_id: req.session_id.clone(),
            // ㉕·对话区①：工具自述进载荷，决策卡理由行的数据源
            description: (!spec.description.is_empty()).then(|| spec.description.clone()),
            // v2.49：决策卡倒计时数据源（0 = 不限，卡片不渲染倒计时）
            timeout_secs: self.timeout_secs(),
            // ㉘·B/㉘·C：环境行如实 + 硬门两键卡（前端藏「允许本次对话」）
            workspace,
            external,
            sensitive: sensitive.map(str::to_string),
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

        // 工具行状态前置补投（CF §2.1，后审计队列 Q12·C1）：审批请求一发出，
        // 那条在途调用就在行上显示「等待审批」，不必等结果才出现。
        // **只走实时通道、不进会话日志**——重放时那次的决策早已落成
        // `ApprovalDecided`，行按"经审批放行/拒绝"的事实闭合，不虚构等待态
        // （一份事实只许一个出处：等待态的真源是"卡片正挂着"这件事本身）。
        // `req.id` 与载荷里的 `request_id` 同串（审批与工具调用同一标识）。
        if let Some(session_id) = req.session_id.clone() {
            self.sink.emit(&AgentEvent::ToolStatus {
                session_id,
                call_id: req.id.0.clone(),
                status: "pending_approval".into(),
            });
        }

        // 守夜任务（§3.17 超时自动拒绝）：抢到移除权 = 用户未决策，
        // fail-closed 拒绝并唤醒挂起的 step；decide 先移除则此处为空转。
        // 表句柄是 Arc 克隆，任务不借用 self。超时 0（设置"不限"）不起
        // 任务——审批点回到可无限等待的原语义。
        let timeout_ms = self.timeout_ms.load(Ordering::Relaxed);
        if timeout_ms > 0 {
            let pending = Arc::clone(&self.pending);
            let request_id = req.id.0.clone();
            tokio::spawn(async move {
                tokio::time::sleep(Duration::from_millis(timeout_ms)).await;
                let entry = pending
                    .lock()
                    .expect("approval pending poisoned")
                    .remove(&request_id);
                if let Some(entry) = entry {
                    let _ = entry.tx.send(ApprovalDecision::deny(format!(
                        "{TIMEOUT_REASON_PREFIX}（{} 秒未响应，fail-closed）",
                        timeout_ms / 1000
                    )));
                }
            });
        }

        // 用户未决策（或未超时）前 step 挂起：工具链上的 Timeout 只包
        // 执行，不包审批（§7.1）；超时行为由上面的守夜任务承担。
        match rx.await {
            Ok(decision) => decision,
            Err(_) => ApprovalDecision::deny("审批通道已关闭（请求被丢弃）"),
        }
    }
}
