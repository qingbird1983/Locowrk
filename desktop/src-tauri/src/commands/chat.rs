//! 会话与轮次命令（DESIGN §12.1 / §12.2）：跑一轮、停一轮、回退一次压缩、读会话
//! 列表与全量投影。业务全在 runner / session_log，本文件只做参数转换与状态
//! 存取（AGENTS §3「command 是薄适配层」）。

use std::sync::Arc;

use tauri::State;

use session_store::{SessionEvent, SessionMeta};

use crate::events::AgentEvent;
use crate::runner::{self, UsagePolicy};
use crate::state::AppState;

/// 一次轮次与一次手动压缩**共用的用量口径**（S2 刀⑦ 从 `agent_chat` 里提出：
/// `/压缩` 也要吃这把尺，抄第二遍就是两个出处——刀① 那条纪律的反面）。
///
/// S2 刀①（2026-10-19）：进度条分母改吃**有效阈值** = `min(滑杆, 窗口 − 输出侧
/// 预留)`，算式只住 `context_budget` 一处。为什么必须一起改：分母与压缩触发线
/// 本来就是同一把尺（v2.138「看到的 = 触发用的」），而滑杆允许设得比窗口还高
/// ——设置行当时的提示「需低于模型窗口，否则上下文超限会先于压缩发生」承认的就是
/// 这个洞（现文案已改为「实际触发线会与模型窗口 − 21K 取小」）；封顶之后洞自动
/// 关闭。**缺省设置下读数不变**（滑杆 100K < 窗口 179K
/// 时封顶取不到小者，`min` 拿回的还是滑杆），只有滑杆高过窗口才生效。
/// 批 F-24b / M-18c：不为算几个数而全量克隆档位区（含各档明文 Key）——一次取锁 +
/// 借用进闭包；闭包内不碰 Store 的其它读面（非重入锁）。
///
/// **S3 刀④a · 收挂账 ⓛ**：四枚原料的装配口从本函数体内搬到
/// [`UsagePolicy::assemble`]，定时任务那条链（`scheduler/engine.rs`）现在调**同一个**
/// assemble —— 原先"两处各写一遍同样的三枚"是刀① 当时的既有双份，ⓟ 加第四枚
/// （拒收读数）之后不收了就会变成"分子的出处也双份"。本函数只剩取原料：窗口/输出
/// 上限走四层链，滑杆走设置，目录估算走 kernel，拒收读数走 `state.compactor`
/// （它是那张表唯一的宿主侧读口，理由见 [`crate::compact::deep::Compactor::refusal`]）。
pub(crate) async fn usage_policy(state: &AppState, session_id: &str) -> UsagePolicy {
    let ov = state.model_overrides.get(session_id);
    let (window, max_output) = state.settings.with_llm_config(|llm| {
        crate::settings::session_capacity(llm, ov.as_ref(), Some(&state.catalog))
    });
    UsagePolicy::assemble(
        state.settings.compact_threshold_tokens(),
        window,
        max_output,
        runner::catalog_estimate(&state.kernel, session_id).await,
        state.compactor.refusal(session_id),
    )
}

/// 聊天命令。轮次内容全部经 `agent://` 事件推送（§12.2），返回值只表达
/// 轮次是否成功收束（返回实际 step 数）。用量口径（v2.137）：阈值 =
/// 压缩阈值（绝对 token）；窗口 = 会话模型四层链自动识别（模型行 ??
/// 目录 ?? 档位，`session_window`）。`parts`（批 D）：附件部件
/// （asset_put 已上传的内容寻址引用；缺省 = 纯文本消息）。
#[tauri::command]
pub async fn agent_chat(
    session_id: String,
    message: String,
    workspace: Option<String>,
    parts: Option<Vec<session_store::ContentPart>>,
    state: State<'_, AppState>,
) -> Result<u32, String> {
    // ㉘·A·fix 归属先行：首聊挂起归属（chat.pendingWorkspace）随请求带入，
    // 建会话即写旁车——先于本轮任何注入 / 围栏。旧时序（轮次收束
    // 后才 session_associate）让首轮注入落在未归属键位、还被 M1 预取缓存
    // 整场（Glance-main 会话引用 Locowrk 旧记忆的实机事故根因）。
    if let Some(ws) = workspace
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        crate::commands::sessions::ensure_associatable(&state.settings.fs_roots(), ws)?;
        crate::session_log::associate(&state.sessions_dir, &session_id, Some(ws))
            .map_err(|e| format!("关联失败: {e}"))?;
    }
    // ⑭：记最近聊过的会话（调度触发的目标解析兜底用）
    state.note_chat(&session_id);
    let session = state.session_lock(&session_id);
    let mut session = session.lock().await;
    let usage = usage_policy(&state, &session_id).await;
    let steps = runner::run_turn(
        &state.kernel,
        &mut session,
        &session_id,
        message,
        parts.unwrap_or_default(),
        Some(usage),
        state.sink.as_ref(),
        Some(state.usage.as_ref()),
        Some(&state.sessions_dir),
        state.settings.max_turn_steps(),
        Some(&state.stop_flags),
        Some(state.fragments.as_ref()),
        Some(&state.compactor),
    )
    .await?;
    // v2.57 自动标题：旁车无手动标题时一次 LLM 调用生成（fire-and-forget，
    // 消息快照带出会话锁，不阻塞返回值——⑬ extract 同款）
    crate::title::spawn_auto_title(
        state.app.clone(),
        Arc::clone(&state.settings),
        state.sessions_dir.clone(),
        session_id.clone(),
        session.derive_messages(),
    );
    Ok(steps)
}

/// 停止生成（v4.11 §3.39④）：四路同收——① 停止标记置位（流式 delta 点
/// 带部分正文收束 + runner 步间边界 TurnDone）；② 挂起的审批卡 fail-closed
/// 拒绝（与守夜超时同方向，留 ApprovalDecided 审计）；③ 该会话全部待决
/// ask 卡按跳过收束；④ 该会话**在途的工具调用**逐个置取消标志
/// （specs/04 · C1）。幂等：轮未在跑 = 各路空转。
///
/// ④ 的诚实边界（现在能做到的全部）：置位之后，只有**宿主内建的 Rust 工具**在
/// 自己的轮询点上看得见这个标志（第三方插件隔着冻结的 C ABI 看不见，那是 C3；
/// `tool-shell` / `tool-fetch` 这类长任务真去查是 C4）。所以这一路现在的效果是
/// "信号送达"，不是"调用立刻停"——日志里那句条数说的是置位了几条，不是打断了几条。
#[tauri::command]
pub async fn agent_stop(session_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.stop_flags.request(&session_id);
    state.approval.deny_session(&session_id, "用户停止了任务");
    state.ask.skip_session(&session_id);
    // ④：置位在途调用的取消标志，并把条数落一行日志（specs/04 C4 要的叙事读数）。
    let cancelled = state.kernel.active_calls().cancel_session(&session_id);
    if cancelled > 0 {
        tracing::info!(session_id = %session_id, cancelled, "停止时置位了在途工具调用");
    }
    Ok(())
}

/// 撤销最近一次上下文压缩（Context and Memory.md §3.7）：rewind_to(压缩
/// 事件序号) 后 fork 原地换入——前缀游标语义下其后的轮次一并回退。⑲ 起
/// 磁盘镜像随 fork 同步重写（滚动备份 `.undo.bak` = 撤销的物理留痕）。
/// 会话 id 不变，前端无需重新绑定。返回回退点（一般无需使用）。
#[tauri::command]
pub async fn context_undo_compact(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let session = state.session_lock(&session_id);
    let mut session = session.lock().await;
    let seq = session.events()[..session.cursor()]
        .iter()
        .rposition(|e| matches!(e, SessionEvent::ContextCompacted { .. }))
        .ok_or_else(|| "没有可撤销的压缩记录".to_string())?;
    session.rewind_to(seq);
    *session = session.fork(seq);
    crate::session_log::rewrite_after_undo(&state.sessions_dir, &session_id, &session);
    Ok(seq)
}

// ---- 会话持久化命令（⑲）：列表与全量投影。历史恢复不走事件通道
// （agent:// 只服务当轮增量，§4.2）。 ----

/// 会话列表（`sessions/` 目录按 mtime 降序；空会话无文件不出现——
/// 首条消息发出前不创建任何后端状态）。
#[tauri::command]
pub async fn session_list(state: State<'_, AppState>) -> Result<Vec<SessionMeta>, String> {
    // 台账 T6：这条命令是本仓最重的一次同步 IO（read_dir + 每个会话逐行扫日志
    // 数事件 + 逐个读旁车 + 排序）。以前它直接在 async 命令线程上跑 = 占住一个
    // tokio worker，会话一多就把并发请求一起拖住（同一族的 assets / llm_import
    // 早就走 spawn_blocking，这里补齐，不是新发明）。
    let dir = state.sessions_dir.clone();
    let metas = tokio::task::spawn_blocking(move || crate::session_log::list(&dir))
        .await
        .map_err(|e| format!("会话列表读取失败: {e}"))?;
    Ok(metas)
}

/// 会话全量投影：整个日志按 runner 同一映射转 UI 事件序列，前端重放
/// 恢复气泡/工具卡/系统通知。会话不存在 = 空表（新会话走正常聊天流）。
#[tauri::command]
pub async fn session_history(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AgentEvent>, String> {
    let Some(lock) = state
        .sessions
        .lock()
        .expect("sessions poisoned")
        .get(&session_id)
        .cloned()
    else {
        return Ok(Vec::new());
    };
    let session = lock.lock().await;
    Ok(runner::project_events(session.events(), &session_id))
}
