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
        // ㉘·C 决策表（DESIGN §7.2，序即优先级）：①计划档硬拒 ②硬门（隐私
        // + 越界）③模式放行（一等公民）④会话豁免 ⑤决策卡。modes 缺席 =
        // 退化为④⑤（旧行为；kernel-bin 桥不装配决策表）。
        let mode = self
            .modes
            .as_ref()
            .map(|m| m.mode_of(req.session_id.as_deref()));
        if mode == Some(crate::permission_mode::PermissionMode::Plan)
            && !matches!(spec.side_effect, plugin_api::SideEffect::Pure)
        {
            // 正常路径已被目录级围栏挡住（非 Pure 不出目录），此处纵深防御
            return ApprovalDecision::deny("计划模式只读：不可写/不可逆调用一律拒绝");
        }

        // ② 硬门：隐私门（privacy.rs 清单，任何档位都拦——完全访问也不例外，
        // 用户裁定"最危险的触及用户隐私的或本地隐私的有拦截"）+ 越界门
        // （㉘·B 外部路径门；围栏改全局声明根后 = 越出声明根才触发）。
        // 命中即强制弹卡且只许一次（不进豁免集、模式放行被跳过）。
        let sensitive = match &self.modes {
            Some(_) => crate::privacy::check(&req.tool, &req.args),
            None => None,
        };
        let external = match (&self.modes, &self.ctx_of) {
            (Some(_), ctx) => command_touches_outside(
                &req.tool,
                &req.args,
                req.session_id.as_deref(),
                ctx.as_ref(),
            ),
            _ => false,
        };
        if sensitive.is_none() && !external {
            // ③ 模式放行（一等公民，㉘·C；审计 reason 带 mode: 前缀，前端
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
            // ④ 会话豁免：本会话已放行过同工具，直接放行（Kernel 照写
            // ApprovalDecided，投影出记录条——转录里留得住"自动批过"）
            let exempt_key = (req.session_id.clone().unwrap_or_default(), req.tool.clone());
            if self
                .exempt
                .lock()
                .expect("approval exempt poisoned")
                .contains(&exempt_key)
            {
                return ApprovalDecision::allow();
            }
        }

        let workspace = self
            .ctx_of
            .as_ref()
            .and_then(|ctx| req.session_id.as_deref().and_then(|sid| ctx(sid).workspace));
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

/// 命令类工具（`shell` / `sandbox_run`）的命令文本是否触及全局声明根之外
/// 的绝对路径（㉘·C 起围栏 = 全局声明根，与插件侧 fsutil 同源——声明根内
/// 跨项目畅通不弹卡）。结构化路径工具（fs/edit/git）不走这里——它们在
/// 插件内对出围栏路径 fail-closed 拒绝（既有契约，拒绝文案引导改走 shell
/// 审批路）。启发式：只识别 Windows 绝对路径形态（盘符 `X:\`·`X:/` 与
/// UNC `\\…`）；扫不出绝对路径 = 视为围栏内——漏报在 `询问` 档仍有弹卡
/// 兜底（shell 本就是 Irreversible），宁漏勿错杀。
pub(crate) fn command_touches_outside(
    tool: &str,
    args: &serde_json::Value,
    session_id: Option<&str>,
    ctx: Option<&crate::workspace::SessionContextResolver>,
) -> bool {
    if !matches!(tool, "shell" | "sandbox_run") {
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
    let roots = scan_roots(session_id, ctx);
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
            i += 1;
        }
    }
    out
}

fn rest_starts_unc(cmd: &str, i: usize) -> bool {
    cmd[i..].starts_with("\\\\")
}

/// 扫描围栏（与插件侧 fsutil 语义逐值对齐，㉘·C）：会话上下文带 `fs_roots`
/// （= 全局声明根 `[数据根] ∪ settings.fsRoots`，`workspace.rs` resolver
/// 下发）就用它；空（未归属/headless）回落 env 全局（数据根 +
/// `LOCOWRK_FS_ROOTS`，即 `fsutil::allowed_roots` 同源）——同一集合。
pub(crate) fn scan_roots(
    session_id: Option<&str>,
    ctx: Option<&crate::workspace::SessionContextResolver>,
) -> Vec<PathBuf> {
    let data_root = plugin_api::fsutil::data_root()
        .ok()
        .map(|p| Path::new(&p).to_path_buf());
    if let (Some(ctx), Some(sid)) = (ctx, session_id) {
        let fs_roots = ctx(sid).fs_roots;
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
        t != r && !t.starts_with(&format!("{r}\\"))
    })
}
