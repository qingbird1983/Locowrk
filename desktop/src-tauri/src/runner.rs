//! 聊天轮次主循环（§12.1 `agent_chat` 的业务所在：命令层保持薄适配）。
//!
//! 每轮：用户消息进事件日志 → Kernel::step 循环（ReAct，§7.2）→
//! 把日志新增段投影成 `agent://` 事件。UI 事件是会话日志的投影，
//! 不是第二真相源——阶段六的回放/恢复可复用同一投影。
//! ⑨ 起每个 step 后补推 `agent://context-usage`（进度条数据源，
//! 与压缩判定同一估算口径）。
//!
//! 失败约定：内核报错时不发 TurnDone，直接返回 Err（前端以 invoke 的
//! reject 收尾），避免与成功路径的事件重复。步数到顶不是失败（LLM.md
//! v3.0 同批）：TurnDone 正常收束 + TurnCapped 如实告知。

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use kernel_agent::{estimate_tokens, Kernel};
use session_store::{Session, SessionEvent, TurnId, TurnOutcome};

use crate::session_log;
use crate::usage::UsageRecorder;

use crate::events::{side_effect_str, AgentEvent, EventSink};

/// 单轮 step 上限缺省值：防止模型反复发工具调用打转（防御性兜底）。
/// ㉕·微调③ 起实际值可配（settings `tools.maxTurnSteps`，工具与暴露
/// 面板），此常量只作缺省与测试取值。
pub const DEFAULT_MAX_STEPS: u32 = 8;

/// 进度条口径（⑨）：threshold = 窗口 × 百分比（组装点算好），
/// window_tokens 未知传 0（前端按"未设置"处理）。`tools_est_tokens`
/// （v2.58 §3.12）= 本轮实际携带的暴露目录估算（agent_chat 每轮一次
/// `Kernel::catalog_snapshot` 快照；0 = 目录缺席，前端不渲染估算段）。
#[derive(Debug, Clone, Copy)]
pub struct UsagePolicy {
    pub threshold_tokens: u64,
    pub window_tokens: u64,
    pub tools_est_tokens: u64,
}

/// 跑完一轮对话，返回实际 step 数。`max_steps` 是单轮兜底上限（㉕·微调③
/// settings `tools.maxTurnSteps`，每轮取值）。`stats` 是 ⑩③ 使用统计写入端
/// （usage.sqlite3，None = 内置替身测试）。`log` 是 ⑲ 会话落盘目录
/// （`<数据根>/sessions`，None = 不落盘，测试路径）——磁盘是日志的镜像，
/// 用户消息与每个 step 的新增段与投影共用同一游标同步追加。`tree`（M4
/// §3.38②）= 树组装器在场时用量环按组装后口径（目录 + 尾巴）估算，None =
/// 全量历史估算（kernel-bin / 测试路径）。`stop`（v4.11 §3.39④）= 停止
/// 标记注册表（None = 无停止面，headless/测试）：进场登记 RAII 守卫、
/// 所有收束路径自动注销；step 边界查到置位即按 TurnDone 收束（流式内的
/// 截断在 LLM 适配器 delta 回调点，更早）。
#[allow(clippy::too_many_arguments)]
pub async fn run_turn(
    kernel: &Kernel,
    session: &mut Session,
    session_id: &str,
    message: String,
    usage: Option<UsagePolicy>,
    sink: &dyn EventSink,
    stats: Option<&UsageRecorder>,
    log: Option<&Path>,
    max_steps: u32,
    tree: Option<&crate::organize::TreeAssembler>,
    stop: Option<&Arc<crate::stop::StopFlags>>,
) -> Result<u32, String> {
    let _stop_guard = stop.map(|f| f.guard(session_id));
    let log_start = session.len();
    // 轮界事件（§4.1 设计，v2.101 起真正写入）：TurnStart 供重放复原计时
    // 起点（配给紧随的 UserMessage），TurnEnd（各收束路径）供冻结读数——
    // 此前日志里没有任何时刻，切会话/重启后计时行只剩发丝线（目检①）。
    let turn_id = TurnId(
        session
            .events()
            .iter()
            .filter(|e| matches!(e, SessionEvent::TurnStart { .. }))
            .count() as u64
            + 1,
    );
    session.append(SessionEvent::TurnStart { turn_id, ts: now_ms() });
    session.append(SessionEvent::UserMessage { content: message });
    if let Some(dir) = log {
        session_log::append_slice(dir, session_id, session, log_start);
    }
    if let Some(u) = usage {
        record_usage(session, session_id, u, sink, log, tree);
    }

    for steps in 1..=max_steps.max(1) {
        // 停止生成（§3.39④）：步间边界检查——流式内的截断发生在 LLM 适配器
        // （delta 回调点，本步已带部分正文收束）；这里兜住"工具跑完后不再
        // 开下一步"。收束 = TurnDone 正常闭环（前端头行按 stopRequested
        // 显示「已停止」，不加系统气泡）。
        if stop.is_some_and(|f| f.is_stopped(session_id)) {
            finish_turn(
                session,
                log,
                session_id,
                turn_id,
                TurnOutcome::Completed,
                Some(sink),
                steps,
            );
            return Ok(steps);
        }
        let before = session.len();
        // ⑭：带会话身份 step——CallCtx.session_id 供内置调度工具绑定
        // 创建会话（§5.2 规划字段；普通 step 保持 None 语义不变）
        let (streamed, reasoned) = match kernel.step_as(session, session_id).await {
            // ⑳/v2.57：streamed / reasoned = 本步正文/推理已经 delta 直推
            // （ctx 里 LlmStreamSink 在册），投影时抑制整段 chunk/Thinking
            // 防重复
            Ok(kernel_agent::StepOutcome::Continue { streamed, reasoned }) => (streamed, reasoned),
            Err(e) => {
                // 轮以失败收束也要落日志（TurnOutcome::Failed）——重放才
                // 知道轮停在哪儿；TurnDone 照失败约定不发（前端以 invoke
                // 的 reject 收尾，避免与成功路径的事件重复）
                finish_turn(
                    session,
                    log,
                    session_id,
                    turn_id,
                    TurnOutcome::Failed,
                    None,
                    steps,
                );
                return Err(format!("kernel step: {e}"));
            }
        };
        project_new_events(session, before, session_id, sink, streamed, reasoned);
        if let Some(dir) = log {
            session_log::append_slice(dir, session_id, session, before);
        }
        record_stats(session, before, stats);

        // 收敛判据：本步 assistant 没有再发工具调用 → 轮次结束。从尾部
        // 找**最近一条 AssistantMessage**再判 tool_calls——不能只看
        // last()：v2.58 起 LlmUsage 紧贴 assistant 之后落日志（内核
        // step_inner），报告用量的供应商下 last() 恒为 LlmUsage，轮次
        // 永不收敛、每轮多跑一步（请求以 assistant 结尾，DeepSeek
        // thinking 模式直接 400 拒收，每轮尾部假 error——M1 走查实抓）。
        let done = matches!(
            session.events().iter().rev().find_map(|e| match e {
                SessionEvent::AssistantMessage { tool_calls, .. } => Some(tool_calls.is_empty()),
                _ => None,
            }),
            Some(true)
        );
        if let Some(u) = usage {
            // v2.62：快照落日志 + 实时投影（自带 append_slice 与单发，
            // 下一轮投影窗口 before 已越过本事件，不会重发）
            record_usage(session, session_id, u, sink, log, tree);
        }
        if done {
            finish_turn(
                session,
                log,
                session_id,
                turn_id,
                TurnOutcome::Completed,
                Some(sink),
                steps,
            );
            return Ok(steps);
        }
    }

    // 步数到顶：不再硬报错掐断（LLM.md v3.0 同批裁定）——正常 TurnDone
    // 收束 + TurnCapped 如实告知，已完成的工具成果与转录都保留，用户发
    // 「继续」即可接着跑；调高上限见 设置 → 工具与分发。
    finish_turn(
        session,
        log,
        session_id,
        turn_id,
        TurnOutcome::Completed,
        Some(sink),
        max_steps.max(1),
    );
    sink.emit(&AgentEvent::TurnCapped {
        session_id: session_id.into(),
        steps: max_steps.max(1),
    });
    Ok(max_steps.max(1))
}

/// 轮收束（v2.101）：TurnEnd 落日志（§4.1 轮界事件，带收束时刻）+ 磁盘
/// 同步 + TurnDone 投影（`sink = None` = 内核失败路径按约定不发）。重放
/// 由 [`project_events`] 把 TurnEnd 映成带时刻的 TurnDone——切会话/重启
/// 后计时行读数（「已工作 N 分 N 秒」）可复原。
fn finish_turn(
    session: &mut Session,
    log: Option<&Path>,
    session_id: &str,
    turn_id: TurnId,
    outcome: TurnOutcome,
    sink: Option<&dyn EventSink>,
    steps: u32,
) {
    let ts = now_ms();
    session.append(SessionEvent::TurnEnd { turn_id, outcome, ts });
    if let Some(dir) = log {
        session_log::append_slice(dir, session_id, session, session.len() - 1);
    }
    if let Some(sink) = sink {
        sink.emit(&AgentEvent::TurnDone {
            session_id: session_id.into(),
            steps: Some(steps),
            ts: Some(ts),
        });
    }
}

/// Unix 毫秒（轮界事件时间戳）。
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// 把日志新增段 `[from..]` 投影成 UI 事件（顺序 = 日志顺序）。
/// `suppress_assistant_chunk`（⑳）：本步正文已经流式 delta 直推时跳过
/// 整段 chunk，避免同一正文出现两遍。`suppress_thinking`（v2.57）：推理
/// delta 已直推时同理跳过整段 Thinking。
#[allow(clippy::too_many_arguments)]
pub(crate) fn project_new_events(
    session: &Session,
    from: usize,
    session_id: &str,
    sink: &dyn EventSink,
    suppress_assistant_chunk: bool,
    suppress_thinking: bool,
) {
    for e in project_events(&session.events()[from..], session_id) {
        if suppress_assistant_chunk && matches!(e, AgentEvent::Chunk { .. }) {
            continue;
        }
        if suppress_thinking && matches!(e, AgentEvent::Thinking { .. }) {
            continue;
        }
        sink.emit(&e);
    }
}

/// 事件切片 → UI 事件序列（纯函数）。增量投影（run_turn）与 ⑲ 全量恢复
/// 投影（session_history 命令）共用同一映射——"UI 事件是会话日志的投影"
/// 的单点实现。
pub fn project_events(events: &[SessionEvent], session_id: &str) -> Vec<AgentEvent> {
    let mut out = Vec::new();
    // ApprovalDecided 只有 call_id（㉕·对话区①）：tool 从同切片的 ToolCall
    // 按 call_id 找回——ToolCall 先行落日志（纪律 4），同一步内必在决策之前。
    let mut call_tools: HashMap<String, String> = HashMap::new();
    // TurnStart.ts 配给紧随的 UserMessage（v2.101）：重放计时起点。消息
    // 编辑重发截断留下的孤儿 TurnStart 会被下一个 TurnStart 覆盖（自愈，
    // 不会把旧轮的起点错配给新轮）。
    let mut turn_ts: Option<i64> = None;
    for (pos, e) in events.iter().enumerate() {
        match e {
            SessionEvent::TurnStart { ts, .. } => turn_ts = Some(*ts),
            // ㉕·fix：用户消息进投影（此前落 `_` 被丢，重启后历史会话
            // 只剩模型回复）。实时轮不受影响——run_turn 的投影起点在
            // UserMessage 追加之后，该段只落盘不上事件通道（前端本地回显）。
            // `pos`（v2.57 §3.34）：消息编辑重发的定位键——session_history
            // 传全量数组，pos 即绝对事件位。`ts`（v2.101）：轮起点时刻。
            SessionEvent::UserMessage { content } => {
                out.push(AgentEvent::UserMessage {
                    session_id: session_id.into(),
                    content: content.clone(),
                    pos,
                    ts: turn_ts.take().filter(|t| *t != 0),
                });
            }
            // 推理（v2.57 §3.9）：投影一行 ThinkingRow；实时轮已被流式
            // 消费的 step 由 project_new_events 抑制（重放照常产出）。
            SessionEvent::Thinking { text } => {
                out.push(AgentEvent::Thinking {
                    session_id: session_id.into(),
                    text: text.clone(),
                });
            }
            // 单步用量（v2.58 §3.12）：实时与重放同形——前端按轮聚合；
            // 不需要抑制（无 delta 直推对应物）。
            SessionEvent::LlmUsage {
                input,
                output,
                cache_read,
                cache_write,
                reasoning,
                total,
                duration_ms,
            } => {
                out.push(AgentEvent::LlmUsage {
                    session_id: session_id.into(),
                    input: *input,
                    output: *output,
                    cache_read: *cache_read,
                    cache_write: *cache_write,
                    reasoning: *reasoning,
                    total: *total,
                    duration_ms: *duration_ms,
                });
            }
            SessionEvent::ContextUsage {
                used,
                threshold,
                window_tokens,
                tools_est_tokens,
            } => {
                // v2.62：用量快照重放同形——切会话回来用量环恢复
                out.push(AgentEvent::ContextUsage {
                    session_id: session_id.into(),
                    used: *used,
                    threshold: *threshold,
                    window_tokens: *window_tokens,
                    tools_est_tokens: *tools_est_tokens,
                });
            }
            SessionEvent::AssistantMessage { content, .. } => {
                // 纯工具调用的 assistant 步正文为空，不发空 chunk
                if !content.is_empty() {
                    out.push(AgentEvent::Chunk {
                        session_id: session_id.into(),
                        delta: content.clone(),
                    });
                }
            }
            SessionEvent::ToolCall {
                call_id,
                tool,
                args,
                side_effect,
                plugin_version,
                ..
            } => {
                call_tools.insert(call_id.0.clone(), tool.clone());
                out.push(AgentEvent::ToolCall {
                    session_id: session_id.into(),
                    call_id: call_id.0.clone(),
                    tool: tool.clone(),
                    args: args.clone(),
                    side_effect: side_effect_str(side_effect).into(),
                    plugin_version: plugin_version.to_string(),
                });
            }
            SessionEvent::ToolResult {
                call_id,
                output,
                error,
            } => out.push(AgentEvent::ToolResult {
                session_id: session_id.into(),
                call_id: call_id.0.clone(),
                ok: error.is_none(),
                output: output.clone(),
                error: error.clone(),
            }),
            SessionEvent::ContextCompacted {
                from_tokens,
                to_tokens,
                strategy,
                summary,
                ..
            } => {
                // 摘要全文在会话日志；UI 只拿 200 字符预览
                out.push(AgentEvent::ContextCompacted {
                    session_id: session_id.into(),
                    from_tokens: *from_tokens,
                    to_tokens: *to_tokens,
                    strategy: strategy.clone(),
                    summary_preview: summary.chars().take(200).collect(),
                });
            }
            SessionEvent::ApprovalDecided {
                call_id,
                allowed,
                reason,
            } => {
                // ㉕·对话区① §3.17：决策记录条的唯一来源——实时增量与历史
                // 重放同源，"谁批过什么"重启后也在转录里
                out.push(AgentEvent::ApprovalResolved {
                    session_id: session_id.into(),
                    call_id: call_id.0.clone(),
                    tool: call_tools
                        .get(&call_id.0)
                        .cloned()
                        .unwrap_or_else(|| call_id.0.clone()),
                    allowed: *allowed,
                    reason: reason.clone(),
                });
            }
            SessionEvent::TurnEnd { ts, .. } => {
                // 轮界收束（v2.101）：映成带时刻的 TurnDone——重放据此冻结
                // 计时行读数。steps 日志未记录 = 不带（宁缺勿假）；旧日志
                // 无此事件，重放行为不变。
                out.push(AgentEvent::TurnDone {
                    session_id: session_id.into(),
                    steps: None,
                    ts: (*ts != 0).then_some(*ts),
                });
            }
            _ => {}
        }
    }
    out
}

/// ⑩③ 使用统计（exposure 文档 §3.3）：ToolResult 闭合时按 (tool,
/// plugin_id) 记一次成败——ToolCall 先行落日志（纪律 5），被拒/悬空调用
/// 不计成功。异步通道 + 5s 批量刷盘在 UsageRecorder 内部。
fn record_stats(session: &Session, from: usize, stats: Option<&UsageRecorder>) {
    let Some(stats) = stats else { return };
    let mut pending: HashMap<String, (String, Option<String>)> = HashMap::new();
    for e in &session.events()[from..] {
        match e {
            SessionEvent::ToolCall {
                call_id,
                tool,
                plugin_id,
                ..
            } => {
                pending.insert(call_id.0.clone(), (tool.clone(), plugin_id.clone()));
            }
            SessionEvent::ToolResult { call_id, error, .. } => {
                if let Some((tool, plugin_id)) = pending.remove(&call_id.0) {
                    stats.record(&tool, plugin_id.as_deref(), error.is_none());
                }
            }
            _ => {}
        }
    }
}

/// 进度条口径与压缩判定同源：折叠后的消息用 kernel_agent::estimate_tokens。
/// v2.62：快照**落会话日志**（SessionEvent::ContextUsage）+ 实时单发投影
/// ——重放经 project_events 同形恢复用量环（此前只推实时通道，切会话
/// 回来统计即消失）。下一轮投影窗口 `before` 已越过本事件，不会重发。
fn record_usage(
    session: &mut Session,
    session_id: &str,
    u: UsagePolicy,
    sink: &dyn EventSink,
    log: Option<&Path>,
    tree: Option<&crate::organize::TreeAssembler>,
) {
    // M4 §3.38②：树会话 = 组装后口径（目录 + 尾巴），如实反映模型实际
    // 看到的上下文；legacy / 组装器缺席 = 全量历史估算（口径不变）。
    let used = match tree {
        Some(t) => t
            .context_tokens(session, session_id)
            .unwrap_or_else(|| estimate_tokens(&session.derive_messages())),
        None => estimate_tokens(&session.derive_messages()),
    };
    session.append(SessionEvent::ContextUsage {
        used,
        threshold: u.threshold_tokens,
        window_tokens: u.window_tokens,
        tools_est_tokens: u.tools_est_tokens,
    });
    if let Some(dir) = log {
        session_log::append_slice(dir, session_id, session, session.len() - 1);
    }
    sink.emit(&AgentEvent::ContextUsage {
        session_id: session_id.into(),
        used,
        threshold: u.threshold_tokens,
        window_tokens: u.window_tokens,
        tools_est_tokens: u.tools_est_tokens,
    });
}

/// 暴露目录估算（v2.58 §3.12 工具上下文行）：目录序列化字节 / 4——与
/// 消息估算同一把尺（§3.12 的"约 N tokens"）。
pub async fn catalog_estimate(kernel: &Kernel, session_id: &str) -> u64 {
    let specs = kernel.catalog_snapshot(Some(session_id)).await;
    let bytes: usize = specs
        .iter()
        .map(|s| serde_json::to_string(s).map(|j| j.len()).unwrap_or(0))
        .sum();
    (bytes / 4) as u64
}
