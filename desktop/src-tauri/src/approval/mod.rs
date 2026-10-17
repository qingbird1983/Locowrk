//! 审批中心后端（DESIGN.md §13.2 / §12.1 / 纪律 9）：`ApprovalResolver`
//! 的桌面端实现。**本文件管"问人"这一侧**——请求载荷、挂起等待、豁免集、守夜超时；
//! 七序决策表本身在 [`decide`]（台账 T1 拆开的：决策序变的改动不该路过等待机制的代码）。
//!
//! Kernel 对每次工具调用咨询 `resolve`（㉘·C：决议者在场 = 全量 consult，
//! 含 Pure；§7.1 审批门）；命中弹卡就把请求作为 `agent://approval-request`
//! 事件推给前端，挂起等待 `agent_approve` 命令回填决策。请求/等待不进会话日志——
//! 日志里的 `ApprovalDecided` 由 Kernel 写（纪律 9），本模块只负责"问人"与自动放行。
//!
//! ㉕·对话区①（UI shell doc §3.17，后端为前端服务）两件桌面策略：
//! - **「允许本次对话」豁免集**：`(session_id, tool)` 命中即 `resolve`
//!   直接放行、不再推请求。进程内生命周期 = 会话级，重启后重新问；
//!   Kernel 对每次放行照写 `ApprovalDecided`，转录记录条不缺审计。
//! - **超时自动拒绝**：请求置位即起守夜任务，缺省 100 秒未决策按拒绝
//!   收束（fail-closed）；`decide` 先到先清，守夜抢到移除权才生效。
//!   阈值可配（v2.49 可配置原则）：settings `tools.approvalTimeoutSecs`，
//!   0 = 不限等待（不起守夜任务），保存即 [`ApprovalHub::set_timeout_secs`]
//!   热换入。
//!
//! v2.145（《Permission Model Redesign (ZCode Alignment) v1》）：
//! - **子会话审批改道**：`{parent}-sub-{n}` 会话 id 一律映射回父根会话
//!   再决策——权限档继承父会话（不再回落设置缺省）、「允许本次对话」
//!   豁免跨派生生效（子 id 每次全新曾使豁免永不命中）、卡片工作区取父。
//!   卡片徽章仍带原始子 id（前端标识用）。
//! - **项目粘性规则**（permission_rules）：决策序新增 ③项目拒绝规则
//!   （先于模式放行 = 用户明示拒绝连 Auto 也拦，比 ZCode yolo 先于
//!   deny 更安全，有意偏离记档）与 ⑤项目允许规则（「本项目始终允许」，
//!   shell = 精确命令规则，其余 = 工具级）。
//!
//! 并发安全：pending/exempt 表是 std Mutex（存取不跨 await）；oneshot 的
//! `rx.await` 在锁外，`agent_approve` 也只碰这两张表，不碰会话锁，
//! 与"聊天锁内串行"无死锁交集。

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kernel_agent::ApprovalDecision;
use tokio::sync::oneshot;

use crate::events::{ApprovalRequestPayload, EventSink};

/// 「硬门·越界」那一条决策分支的实现簇（Windows 绝对路径启发式扫描）。
/// 拆出去的理由 = 路径归一与前缀比较自成一套算法，与决策表唯一的耦合点就是
/// `decide.rs` 里那一句调用（批 F-24a 改 memo 时实测）。
mod fence;
// 再导出而不是改调用点与测试的 `use` 路径：`command_touches_outside` 是决策表
// 那一行的直接调用名，`is_outside`/`scan_roots`/`extract_windows_abs_paths` 由
// `tests_approval/m18_equiv.rs`（批 F-24a 的等值矩阵与围栏正向钉）和
// `tests_permission.rs` 按 `crate::approval::…` 引用 ⇒ 测试面一字不改。
pub(crate) use fence::command_touches_outside;
/// 七序决策表（`resolve`）住在 `decide.rs`：那张表变的是"规则次序与裁决文案"，
/// 本文件变的是"请求、挂起、豁免、守夜超时"——两种变化理由分开（台账 T1）。
/// 这里只声明模块：trait 实现不需要调用点，`Pending` 等私有项由子模块直接可见。
mod decide;
// 后三个只有测试在用：收进 `#[cfg(test)]`，否则非测试编译报 `unused_imports`
// （这是拆分后第一次 `cargo check` 实测到的那条，不是猜测）。
#[cfg(test)]
pub(crate) use fence::{extract_windows_abs_paths, is_outside, scan_roots};

/// 守夜拒绝理由前缀：notifications 提取器按它把超时拒绝识别为
/// ApprovalTimeout 通知（共享常量，防两处裸字符串抄写漂移）。
pub const TIMEOUT_REASON_PREFIX: &str = "超时自动拒绝";

/// 审批超时缺省（§3.17 照实机：100 秒未响应自动拒绝）。v2.49 起只是
/// 测试缺省——生产初值来自设置 `tools.approvalTimeoutSecs`（可配置原则
/// §七-16①），经 [`ApprovalHub::set_timeout_secs`] 热换入。
#[cfg(test)]
pub const APPROVAL_TIMEOUT: Duration = Duration::from_secs(100);

struct Pending {
    request: ApprovalRequestPayload,
    tx: oneshot::Sender<ApprovalDecision>,
}

pub struct ApprovalHub {
    sink: Arc<dyn EventSink>,
    pending: Arc<Mutex<HashMap<String, Pending>>>,
    exempt: Mutex<HashSet<(String, String)>>,
    /// 守夜超时（毫秒；内部用 ms 是为了测试注 50ms 这类短时；0 = 不限，
    /// 不起守夜任务——可配置原则"不为用户做死限定"）。
    timeout_ms: AtomicU64,
    /// ㉘·B 会话级权限模式存储（DESIGN §7.2 决策表）。None = 决策表退化
    /// 为既有"豁免 → 决策卡"两步（旧测试 / 最小组装）。
    modes: Option<Arc<crate::permission_mode::PermissionModeStore>>,
    /// ㉘·B 会话→调用期上下文：环境行"如实"数据源（`.workspace`）+ 硬门
    /// （越界）的围栏来源（`.fs_roots`，㉘·C 起与插件围栏同源 = 全局声明根）。
    ctx_of: Option<crate::workspace::SessionContextResolver>,
    /// v2.145 项目粘性授权规则库（决策序 ③deny / ⑤allow）。None = 跳过
    /// 两级（旧测试 / 最小组装）。
    rules: Option<Arc<crate::permission_rules::PermissionRuleStore>>,
}

/// 子会话 id 改道父根：`{parent}-sub-{n}` → `{parent}`（v2.145——模式
/// 继承 / 豁免共享 / 规则键位 / 卡片工作区全部取父根会话；身份徽章仍用
/// 原始 id）。桌面会话 id 形如 `desktop-<millis>`，不含 `-sub-`，无歧义。
pub(crate) fn root_session_id(sid: Option<&str>) -> Option<String> {
    sid.map(|s| match s.split_once("-sub-") {
        Some((parent, _)) if !parent.is_empty() => parent.to_string(),
        _ => s.to_string(),
    })
}

/// 规则匹配的命令文本：目前只有 `shell` 带 `command` 参数。
fn command_subject<'a>(tool: &str, args: &'a serde_json::Value) -> Option<&'a str> {
    if tool == "shell" {
        args.get("command").and_then(serde_json::Value::as_str)
    } else {
        None
    }
}

impl ApprovalHub {
    /// 测试缺省构造（100s）；生产走 `with_timeout`（初值来自设置）。
    #[cfg(test)]
    pub fn new(sink: Arc<dyn EventSink>) -> Self {
        Self::with_timeout(sink, APPROVAL_TIMEOUT)
    }

    /// 测试注入短超时用；生产行为同 `new`（初值来自设置）。
    pub fn with_timeout(sink: Arc<dyn EventSink>, timeout: Duration) -> Self {
        Self {
            sink,
            pending: Arc::new(Mutex::new(HashMap::new())),
            exempt: Mutex::new(HashSet::new()),
            timeout_ms: AtomicU64::new(timeout.as_millis() as u64),
            modes: None,
            ctx_of: None,
            rules: None,
        }
    }

    /// ㉘·B：接会话级权限模式存储（决策表三档门的数据源）。
    pub fn with_modes(mut self, modes: Arc<crate::permission_mode::PermissionModeStore>) -> Self {
        self.modes = Some(modes);
        self
    }

    /// ㉘·B：接会话→调用期上下文解析器（环境行 + 硬门围栏；㉘·C 起传全量
    /// `SessionContextResolver` 而非 workspace-only 视图——越界门要与插件
    /// 围栏同源）。
    pub fn with_workspace(mut self, ctx: crate::workspace::SessionContextResolver) -> Self {
        self.ctx_of = Some(ctx);
        self
    }

    /// v2.145：接项目粘性授权规则库（决策序 ③deny / ⑤allow）。
    pub fn with_rules(mut self, rules: Arc<crate::permission_rules::PermissionRuleStore>) -> Self {
        self.rules = Some(rules);
        self
    }

    /// 会话（父根）→ 工作区。None = 未归属 / 无 resolver。
    fn workspace_of(&self, root_sid: Option<&str>) -> Option<String> {
        let ctx = self.ctx_of.as_ref()?;
        root_sid.and_then(|sid| ctx(sid).workspace)
    }

    /// 测试脚手架：直接给豁免表种一条记录（生产路径经 `decide`）。
    #[cfg(test)]
    pub(crate) fn decide_seed_for_test(&self, session_id: &str, tool: &str) {
        self.exempt
            .lock()
            .expect("exempt poisoned")
            .insert((session_id.into(), tool.into()));
    }

    /// 守夜超时热换入（settings_set 保存即生效）；0 = 不限等待。
    pub fn set_timeout_secs(&self, secs: u64) {
        self.timeout_ms
            .store(secs.saturating_mul(1000), Ordering::Relaxed);
    }

    fn timeout_secs(&self) -> u64 {
        self.timeout_ms.load(Ordering::Relaxed) / 1000
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
    /// `scope = Some("session")` 且放行 → 记 (root_session, tool) 豁免，
    /// 本会话（含其子代理）后续同工具调用不再问（§3.17「允许本次对话」；
    /// v2.145 起按父根会话记键——子会话改道语义，豁免跨派生生效）。
    /// `scope = Some("project")` 且放行 → 落盘粘性规则（v2.145「本项目
    /// 始终允许」：shell = 精确命令规则，其余 = 工具级；未归属会话跳过
    /// 持久化，当次放行仍生效）。
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
        let root = root_session_id(entry.request.session_id.as_deref());
        if allowed && scope == Some("session") {
            let key = (root.clone().unwrap_or_default(), entry.request.tool.clone());
            self.exempt
                .lock()
                .expect("approval exempt poisoned")
                .insert(key);
        }
        if allowed && scope == Some("project") {
            if let (Some(rules), Some(ws)) = (
                &self.rules,
                root.as_deref().and_then(|sid| self.workspace_of(Some(sid))),
            ) {
                let content = if entry.request.tool == "shell" {
                    entry
                        .request
                        .args
                        .get("command")
                        .and_then(serde_json::Value::as_str)
                        .map(str::to_string)
                } else {
                    None
                };
                if let Err(e) = rules.add_allow(&ws, &entry.request.tool, content) {
                    // 持久化失败不回滚当次放行（决策已定），如实告警
                    tracing::warn!("project allow rule persist failed: {e}");
                }
            }
        }
        let decision = if allowed {
            ApprovalDecision::allow()
        } else {
            ApprovalDecision::deny(reason.unwrap_or_else(|| "用户拒绝".into()))
        };
        let _ = entry.tx.send(decision);
        Ok(())
    }

    /// 停止生成 / 会话收尾（v4.11 §3.39④）：该会话（含子 Agent
    /// `{parent}-sub-{n}`）全部待决按**拒绝**收束（fail-closed，与超时
    /// 守夜同向）——不留悬空卡，也不让停止后的轮次继续挂在审批上。
    /// Kernel 照写 `ApprovalDecided`（纪律 9），转录决策记录不缺审计。
    pub fn deny_session(&self, session_id: &str, reason: &str) {
        let prefix = format!("{session_id}-sub-");
        let mut map = self.pending.lock().expect("approval pending poisoned");
        let ids: Vec<String> = map
            .iter()
            .filter(|(_, p)| {
                p.request
                    .session_id
                    .as_deref()
                    .is_some_and(|sid| sid == session_id || sid.starts_with(&prefix))
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            if let Some(entry) = map.remove(&id) {
                let _ = entry.tx.send(ApprovalDecision::deny(reason.to_string()));
            }
        }
    }
}
