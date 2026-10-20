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

use kernel_agent::Kernel;

use session_store::{Session, SessionEvent, TurnId, TurnOutcome};

use crate::session_log;
use crate::usage::UsageRecorder;

use crate::events::{AgentEvent, EventSink};
use crate::timeutil::now_ms;

pub(crate) mod projection;
pub(crate) mod turn_summary;
pub use projection::project_events;
// 轮末汇总（Q12·C1 + 第二刀）：聚合口径住在 `turn_summary`，实时路径经
// [`emit_turn_summary`]（`finish_turn` 唯一调用点）与重放路径（`project_events`
// 的 TurnEnd 支）**调同一批函数**——"两处同一把尺"是形状判据钉住的形状。
use turn_summary::emit_turn_summary;

/// 单轮 step 上限缺省值：防止模型反复发工具调用打转（防御性兜底）。
/// ㉕·微调③ 起实际值可配（settings `tools.maxTurnSteps`，工具与暴露
/// 面板），此常量只作缺省与测试取值。
pub const DEFAULT_MAX_STEPS: u32 = 8;

/// 进度条口径（⑨）：threshold = 窗口 × 百分比（组装点算好），
/// window_tokens 未知传 0（前端按"未设置"处理）。`tools_est_tokens`
/// （v2.58 §3.12）= 本轮实际携带的暴露目录估算（agent_chat 每轮一次
/// `Kernel::catalog_snapshot` 快照；0 = 目录缺席，前端不渲染估算段）。
/// `refused_tokens`（**S3 刀④a · ⓟ**）= 超窗拒收读数（`compact::Overflow`
/// 表里这一会话最近一次被拒的输入量，没有传 0）。为什么必须由组装点填进来
/// 而不是进度条自己去查：那张表的主人是压缩层，runner 层去抓它就等于给同一个
/// 数开第二个通路；组装点本来就在读同一个 `Arc`（`Compactor` 吃的就是它），
/// 顺手取一次即可。**分子算式不在本结构里** —— 三档取最大住在
/// [`crate::context_budget::occupancy`]，判据 SS30 钉它只许有一处定义。
#[derive(Debug, Clone, Copy)]
pub struct UsagePolicy {
    pub threshold_tokens: u64,
    pub window_tokens: u64,
    pub tools_est_tokens: u64,
    pub refused_tokens: u64,
}

impl UsagePolicy {
    /// **两个组装点共用的构造器**（S3 刀④a · 收 `specs/09` 挂账 ⓛ）：聊天
    /// （`commands/chat.rs::usage_policy`）与定时任务（`scheduler/engine.rs`）
    /// 原先各写一遍同样的"四枚原料 → 一把尺"。ⓟ 把分子收成 `occupancy` 一个函数
    /// 之后，这份重复的性质变了：改前漂的是**分母**（阈值算式），现在还会漂**分子**
    /// （`refused_tokens` 那一格），而定时任务会话同样会撞超窗 ⇒ 漂了就是那条链的
    /// 进度永久低估。算式本体仍只住 [`crate::context_budget`]，本函数只是装配口。
    ///
    /// 参数按"原料"给而不吃容器（`AppState` / `EngineDeps`）：调度引擎在 state
    /// 之外构造，吃容器就得让 runner 反向依赖组装点。
    pub(crate) fn assemble(
        slider: u64,
        window: Option<u64>,
        max_output: Option<u64>,
        tools_est_tokens: u64,
        refused_tokens: u64,
    ) -> Self {
        Self {
            threshold_tokens: crate::context_budget::effective_threshold(
                slider, window, max_output,
            ),
            window_tokens: window.unwrap_or(0),
            tools_est_tokens,
            refused_tokens,
        }
    }
}

/// 跑完一轮对话，返回实际 step 数。`max_steps` 是单轮兜底上限（㉕·微调③
/// settings `tools.maxTurnSteps`，每轮取值）。`stats` 是 ⑩③ 使用统计写入端
/// （usage.sqlite3，None = 内置替身测试）。`log` 是 ⑲ 会话落盘目录
/// （`<数据根>/sessions`，None = 不落盘，测试路径）——磁盘是日志的镜像，
/// 用户消息与每个 step 的新增段与投影共用同一游标同步追加。（这一串参数里
/// 原来还有一枚 `tree`（M4 §3.38②）：树组装器在场时用量环按**组装后口径**
/// （目录 + 尾巴）估算。S3 刀④a 连定义带参数摘掉——它唯一的消费者就是进度条，
/// 而 ⓟ 要的是「进度条与压缩触发同一个分子」；留着它就得让分子有两个算法。
/// 读数语义变化见 [`crate::context_budget`] 头注。）`stop`（v4.11 §3.39④）= 停止
/// 标记注册表（None = 无停止面，headless/测试）：进场登记 RAII 守卫、
/// 所有收束路径自动注销；step 边界查到置位即按 TurnDone 收束（流式内的
/// 截断在 LLM 适配器 delta 回调点，更早）。`organizer`（队列 Q8b）= 轮内水位
/// 泄压的写侧通路（None = headless/测试，与改前逐字节同形）：**每个 step 的
/// 边界**问一次「占用到水位了没、这批历史该不该归档」，命中就同步做一次规则
/// 切分（不调模型）后接着跑——轮末的 `after_turn` 追不上本轮的第 N 个 step，
/// 真机那 63 万 token 就是这么攒出来的。`compactor`（S2 刀④）= 主层深度压缩（None =
/// headless/测试，与改前逐字节同形）：紧接在泄压**之后**、`kernel.step_as` **之前**问
/// 一次「占用到有效阈值了没、机械层压完还超吗」，到线就调一次摘要模型、把较旧的一段
/// 折成一条 `ContextCompacted` 落进日志（为什么挂这儿而不是挂 `agent/request` 环上 =
/// `compact.rs` 头注那三条；为什么排在泄压之后 = 上面那句裁定 C）。
#[allow(clippy::too_many_arguments)]
pub async fn run_turn(
    kernel: &Kernel,
    session: &mut Session,
    session_id: &str,
    message: String,
    parts: Vec<session_store::ContentPart>,
    usage: Option<UsagePolicy>,
    sink: &dyn EventSink,
    stats: Option<&UsageRecorder>,
    log: Option<&Path>,
    max_steps: u32,
    stop: Option<&Arc<crate::stop::StopFlags>>,
    fragments: Option<&crate::fragments::FragmentPending>,
    organizer: Option<&Arc<crate::organize::Organizer>>,
    compactor: Option<&Arc<crate::compact::deep::Compactor>>,
) -> Result<u32, String> {
    let _stop_guard = stop.map(|f| f.guard(session_id));
    let log_start = session.len();
    // 本轮已泄压次数（上限 `developer.organizeMaxRelease` 在 pressure 侧比对）。
    // **故意是局部变量而不是投影字段**：它的生命周期就是这一轮，落盘只会多出
    // 一份跨轮脏账（轮被停止/失败掐断时那份账没人清）。
    let mut released: u32 = 0;
    // 轮界事件（§4.1 设计，v2.101 起真正写入）：TurnStart 供重放复原计时
    // 起点（配给紧随的 UserMessage），TurnEnd（各收束路径）供冻结读数——
    // 此前日志里没有任何时刻，切会话/重启后计时行只剩发丝线（目检①）。
    // 批 F-24b / M-18d：改前是「全量重扫数 `TurnStart` 条数 + 1」⇒ 每轮 O(n)、全会话
    // O(n²)；现在反向找**最后一个 `TurnStart` 自带的 id** 再 +1（O(1) 均摊）。
    // ⚠ 这一条**有行为变化，且方向是修 bug**，分叉条件不是"编辑重发截断"（那是前缀截断，
    //   两式恒等）而是**日志缺轮界行**：`load_session` 逐行读 jsonl、坏行跳过（skipped>0），
    //   于是「`TurnStart` 条数」可以小于「最后一个 `TurnStart` 的 id」⇒ 旧算法会算出一个
    //   **与日志里已有轮号重复**的号（计时行与冻结读数都按 `TurnId` 配 = 新轮读数冻到旧轮），
    //   新算法单调不回退。专测 = `tests_hotpath.rs`，驱动 `run_turn` 本体。
    let turn_id = TurnId(
        session
            .events()
            .iter()
            .rev()
            .find_map(|e| match e {
                SessionEvent::TurnStart { turn_id, .. } => Some(turn_id.0),
                _ => None,
            })
            .unwrap_or(0)
            + 1,
    );
    session.append(SessionEvent::TurnStart {
        turn_id,
        ts: now_ms(),
    });
    session.append(SessionEvent::UserMessage {
        content: message,
        parts,
    });
    if let Some(dir) = log {
        session_log::append_slice(dir, session_id, session, log_start);
    }
    if let Some(u) = usage {
        record_usage(session, session_id, u, sink, log);
    }

    for steps in 1..=max_steps.max(1) {
        // 停止生成（§3.39④）：步间边界检查——流式内的截断发生在 LLM 适配器
        // （delta 回调点，本步已带部分正文收束）；这里兜住"工具跑完后不再
        // 开下一步"。收束如实记 `Interrupted`（Q12·第二刀 v2.205）：取证批实测过
        // 旧形状里停止也走 `Completed` ⇒「已停止」与正常完成在日志中**同形**，
        // 前端只能靠本地 `stopRequested` 自算、切会话/重启后那个标记就没了。现在
        // 权威态进日志（旧日志读回来仍是 `Completed` = 不事后猜它当时是不是被停的）。
        if stop.is_some_and(|f| f.is_stopped(session_id)) {
            finish_turn(
                session,
                log,
                session_id,
                turn_id,
                TurnOutcome::Interrupted,
                Some(sink),
                steps,
            );
            return Ok(steps);
        }
        // 轮内水位泄压（队列 Q8b）：**必须在 `kernel.step_as` 之前**——这一步
        // 就要发出去的请求才是被泄压救下的那一个（挪到 step 之后 = 本轮第一个
        // 请求照旧超限，只有第二个开始才受益，等于把救援推迟了一整步）。
        // `log_start` 是本轮进场时的日志长度 = 切口的上界，本轮的用户消息与
        // 后续 step 的产物一条都不会被归进章节。
        if let (Some(org), Some(u)) = (organizer, usage) {
            if org
                .release_pressure(session, session_id, u.window_tokens, released, log_start)
                .chapters()
                > 0
            {
                released += 1;
            }
        }
        // 主层深度压缩（S2 刀④）：**同样在 `kernel.step_as` 之前，但在泄压之后**。
        // 这一条序是裁定 C 的另一半：机械层（泄压 = 纯 CPU 的规则切分，不调模型）
        // 能省下一条模型往返就该先省，主层只在"机械层压完仍到线"时才起步——反过来
        // 排的话，每个 step 边界都会先烧一条摘要，再发现泄压本来就够了。
        // 落盘单独一趟：`ContextCompacted` 必须在本步之前就已在磁盘上（内核下一步的
        // 投影与崩溃重放都要看得见它），而下面那个 `before` 是"本步新增段"的起点，
        // 两个游标不能混用。
        let compact_start = session.len();
        if let (Some(cp), Some(u)) = (compactor, usage) {
            let out = cp.run(session, session_id, &u).await;
            if let Some(dir) = log {
                session_log::append_slice(dir, session_id, session, compact_start);
            }
            tracing::debug!(session = session_id, ?out, "步间主层压缩读数");
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
            record_usage(session, session_id, u, sink, log);
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
    // 尾部碎片（提示词分层设计 §4.4）：下一轮开场告知模型「到顶了、成果
    // 保留、用户可发继续」——到顶对模型不再静默。
    if let Some(f) = fragments {
        f.set(session_id, crate::fragments::TURN_CAPPED);
    }
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
    session.append(SessionEvent::TurnEnd {
        turn_id,
        outcome,
        ts,
    });
    if let Some(dir) = log {
        session_log::append_slice(dir, session_id, session, session.len() - 1);
    }
    if let Some(sink) = sink {
        // 轮末汇总（`TurnHeader` → `FileChanges`）整体交给 [`emit_turn_summary`]：
        // 区间起点、墙钟起点、审批等待时长都由日志现推，`finish_turn` 不再自己切区间
        // ——实时与重放因此共用同一批算式，参数面也停在 7 颗（第八颗会撞
        // `clippy::too_many_arguments`，上一刀已立「让日志自己回答」的取向）。
        // 两条都早于 TurnDone：前端把轮头与清单挂在轮尾，TurnDone 一到就冻结读数。
        emit_turn_summary(session, session_id, turn_id, outcome, ts, sink);
        sink.emit(&AgentEvent::TurnDone {
            session_id: session_id.into(),
            steps: Some(steps),
            ts: Some(ts),
        });
    }
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

/// 供应商真实 input（日志最后一条 LlmUsage 的 prompt_tokens，含缓存命中
/// ——模型实际收到的上下文；无上报 = None）。v2.138 起这是用量面的唯一
/// 真尺；S3 刀④a 之后它是**锚点的第 1 档**（[`crate::context_budget::anchor`]
/// 的取数通道），进度条不再绕过锚点直接吃它——原来"进度条也吃这把尺"的说法
/// 仍然成立，只是通路改经 `occupancy`。
pub(crate) fn reported_input_tokens(session: &Session) -> Option<u64> {
    session.events().iter().rev().find_map(|e| match e {
        SessionEvent::LlmUsage { input, .. } => Some(*input),
        _ => None,
    })
}

/// 进度条快照（**S3 刀④a · ⓟ：分子与压缩触发同吃一把尺**）。
///
/// `used` 一律取自 [`crate::context_budget::occupancy`] —— 改前这里是**第二把尺**：
/// 优先供应商真实 input，无上报时对树会话回落「目录 + 尾巴」（组装后口径）、
/// 非树会话回落全量历史估算，而压缩层的分子另有其式（压后投影 + 拒收读数）。
/// 两把尺的后果是「进度条离触发线还差一截却已经压缩」和反过来的「看着满线不动
/// 却一直在压」，而且同一个会话换个模型就换一种曲线。现在两个消费点只剩一次
/// 取数：本函数与 `Compactor::used` 都调 `occupancy`（判据 SS30 钉死）。
///
/// ⚠ **这是读数语义变更，不是修 bug**：无供应商上报的树会话从此按全量历史估算，
/// 曲线**只会变高不会变低**（目录段已被 S3 刀③a 从上下文里摘掉，「目录 + 尾巴」
/// 本来就是它的下界）。让曲线变高的同时让它更早触发的方向是安全的（提前压缩 ≠
/// 超限拒收），所以在真机读数拿到之前不视为回归——登记在 specs/10 的 ⓟ 那条。
///
/// v2.62：快照**落会话日志**（SessionEvent::ContextUsage）+ 实时单发投影——重放
/// 经 project_events 同形恢复用量环（此前只推实时通道，切会话回来统计即消失）。
/// 下一轮投影窗口 `before` 已越过本事件，不会重发。
fn record_usage(
    session: &mut Session,
    session_id: &str,
    u: UsagePolicy,
    sink: &dyn EventSink,
    log: Option<&Path>,
) {
    let used = crate::context_budget::occupancy(session, u.refused_tokens);
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
