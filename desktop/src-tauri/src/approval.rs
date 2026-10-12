//! 审批中心后端（DESIGN.md §13.2 / §12.1 / 纪律 9）：`ApprovalResolver`
//! 的桌面端实现。
//!
//! Kernel 对每次工具调用咨询 `resolve`（㉘·C：决议者在场 = 全量 consult，
//! 含 Pure；§7.1 审批门）；这里按 §7.2 决策表裁决——命中弹卡就把请求作为
//! `agent://approval-request` 事件推给前端，挂起等待 `agent_approve` 命令
//! 回填决策。请求/等待不进会话日志——日志里的 `ApprovalDecided` 由
//! Kernel 写（纪律 9），本模块只负责"问人"与自动放行。
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
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use kernel_agent::{ApprovalDecision, ApprovalResolver, ToolCallReq};
use plugin_api::ffi::CallContext;
use plugin_api::ToolSpec;
use tokio::sync::oneshot;

use crate::events::{side_effect_str, AgentEvent, ApprovalRequestPayload, EventSink};

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

// ---- 硬门·越界（㉘·B 外部路径门，启发式命令扫描）---------------------------

/// 命令类工具（`shell`）的命令文本是否触及全局声明根之外
/// 的绝对路径（㉘·C 起围栏 = 全局声明根，与插件侧 fsutil 同源——声明根内
/// 跨项目畅通不弹卡）。结构化路径工具（fs/edit/git）不走这里——它们在
/// 插件内对出围栏路径 fail-closed 拒绝（既有契约，拒绝文案引导改走 shell
/// 审批路）。启发式：只识别 Windows 绝对路径形态（盘符 `X:\`·`X:/` 与
/// UNC `\\…`）；扫不出绝对路径 = 视为围栏内——漏报在 `询问` 档仍有弹卡
/// 兜底（shell 本就是 Irreversible），宁漏勿错杀。
/// （v2.145：`sandbox_run` 随插件退役移出扫描面。）
pub(crate) fn command_touches_outside(
    tool: &str,
    args: &serde_json::Value,
    ctx: Option<&CallContext>,
) -> bool {
    if tool != "shell" {
        return false;
    }
    let cmd = args
        .get("command")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    let tokens = extract_windows_abs_paths(cmd);
    if tokens.is_empty() {
        return false;
    }
    let roots = scan_roots(ctx);
    // 围栏空（连数据根都判不出）= 无法判定，宁可问一次
    if roots.is_empty() {
        return true;
    }
    tokens.iter().any(|t| is_outside(t, &roots))
}

/// 命令文本里的 Windows 绝对路径 token：`X:\…` / `X:/…` / UNC `\\…`。
/// 分隔符取空格 / 引号 / `;` / `<` `>` `|` `&` / 换行。
pub(crate) fn extract_windows_abs_paths(cmd: &str) -> Vec<String> {
    let is_delim = |c: char| {
        matches!(
            c,
            ' ' | '\t' | '"' | '\'' | ';' | '<' | '>' | '|' | '&' | '\n' | '\r'
        )
    };
    let bytes = cmd.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < cmd.len() {
        let is_abs = rest_starts_unc(cmd, i)
            || (i + 2 < cmd.len()
                && bytes[i].is_ascii_alphabetic()
                && bytes[i + 1] == b':'
                && matches!(bytes[i + 2], b'\\' | b'/'));
        if is_abs {
            let end = cmd[i..].find(is_delim).map(|k| i + k).unwrap_or(cmd.len());
            let token = cmd[i..end].trim_matches(|c| c == '"' || c == '\'');
            if !token.is_empty() {
                out.push(token.to_string());
            }
            i = end;
        } else {
            // 按整字符步进，保 i 恒为 char boundary——字节步进会在多字节
            // 字符（中文命令）的续字节上 cmd[i..] 切片 panic
            i += cmd[i..].chars().next().map_or(1, char::len_utf8);
        }
    }
    out
}

fn rest_starts_unc(cmd: &str, i: usize) -> bool {
    cmd.as_bytes()[i..].starts_with(b"\\\\")
}

/// 扫描围栏（与插件侧 fsutil 语义逐值对齐，㉘·C）：会话上下文带 `fs_roots`
/// （= 全局声明根 `[数据根] ∪ settings.fsRoots`，`workspace.rs` resolver
/// 下发）就用它；空（未归属/headless）回落 env 全局（数据根 +
/// `LOCOWRK_FS_ROOTS`，即 `fsutil::allowed_roots` 同源）——同一集合。
/// M-18a：收**已解析的** `CallContext`（调用方一次决策只解析一次），不再
/// 自己拿 resolver + session_id 去解析。
pub(crate) fn scan_roots(ctx: Option<&CallContext>) -> Vec<PathBuf> {
    let data_root = plugin_api::fsutil::data_root()
        .ok()
        .map(|p| Path::new(&p).to_path_buf());
    if let Some(fs_roots) = ctx.map(|c| &c.fs_roots) {
        if !fs_roots.is_empty() {
            return data_root
                .into_iter()
                .chain(fs_roots.iter().map(PathBuf::from))
                .collect();
        }
    }
    let mut roots: Vec<PathBuf> = data_root.into_iter().collect();
    if let Ok(extra) = std::env::var("LOCOWRK_FS_ROOTS") {
        for seg in extra.split(';') {
            let t = seg.trim();
            if !t.is_empty() {
                roots.push(PathBuf::from(t));
            }
        }
    }
    roots
}

/// token 是否落在所有根之外。归一：`/`→`\`、去 verbatim 前缀、小写
/// （Windows 大小写不敏感）、去尾分隔；前缀比较按分量边界——`f:\ab` 不算
/// `f:\a` 内。
pub(crate) fn is_outside(token: &str, roots: &[PathBuf]) -> bool {
    let norm = |s: &str| -> String {
        let s = s.replace('/', "\\");
        let s = s.strip_prefix("\\\\?\\").unwrap_or(&s);
        s.trim_end_matches('\\').to_ascii_lowercase()
    };
    let t = norm(token);
    roots.iter().all(|r| {
        let r = norm(&r.to_string_lossy());
        // M-18e②：`strip_prefix` 代替 `format!("{r}\\")` + `starts_with`
        // ——少一次堆分配（每个根每次判定都新建一个 String），语义逐值等价：
        // 「t 以 r 开头且紧接一个分隔符」= 「剥掉 r 前缀后剩下的以分隔符开头」；
        // 剥不动（不以 r 开头）= 本就不匹配 ⇒ `is_none_or` 取真。分量边界
        // （`f:\ab` 不算 `f:\a` 内）由「剥完还得是分隔符」这一位
        // 保住，与改前同。
        t != r
            && t.strip_prefix(&r)
                .is_none_or(|rest| !rest.starts_with('\\'))
    })
}
