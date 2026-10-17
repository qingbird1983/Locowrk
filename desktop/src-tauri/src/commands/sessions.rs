//! 会话运营命令（v2.49，UI shell 文档 §3.6 行尾 `…` 菜单的后端）：重命名
//! / 删除 / 搜索 / 导出。§3.16 子转录读取（v2.52）。薄适配层——文件面在
//! session_store::persist（经 session_log 薄封装），投影与渲染在 runner /
//! export，这里只接参数。

use tauri::State;

use session_store::{
    load_session, session_file, SessionEvent, SessionSearchHit, UncertainDecision,
};

use crate::events::AgentEvent;
use crate::runner;
use crate::state::AppState;
use crate::timeutil::now_ms;

/// 会话重命名：写旁车 `<id>.meta.json` 标题（trim 后空 = 回落自动标题）。
/// 内存会话不动，列表层生效。
#[tauri::command]
pub async fn session_rename(
    session_id: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    crate::session_log::rename(&state.sessions_dir, &session_id, &title)
        .map_err(|e| format!("重命名失败: {e}"))
}

/// 归属目标校验（后端直读设置，不信前端转述）：必须在当前 fsRoots 里。
/// `session_associate` 与 `agent_chat` 的归属先行（㉘·A·fix）共用一条防线。
pub(crate) fn ensure_associatable(roots: &[String], ws: &str) -> Result<(), String> {
    if roots.iter().any(|r| r == ws) {
        Ok(())
    } else {
        Err(format!(
            "「{ws}」不在 设置 → 工具 → 工作区目录 里，不能关联"
        ))
    }
}

/// 会话↔工作区关联（v2.50 §2.3）：写旁车 `workspace` 字段；`None`/空 =
/// 移出。目标必须是当前 fsRoots 之一（后端直读设置校验，不信前端转述）；
/// 根目录日后从设置移除的，既有归属不追改（侧栏组名取存储路径，组照常
/// 显示，会话仍可移出）。
#[tauri::command]
pub async fn session_associate(
    session_id: String,
    workspace: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let ws = workspace
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    if let Some(ws) = ws {
        ensure_associatable(&state.settings.fs_roots(), ws)?;
    }
    crate::session_log::associate(&state.sessions_dir, &session_id, ws)
        .map_err(|e| format!("关联失败: {e}"))
}

/// 会话级权限模式（㉘·B，UI shell 文档 §3.33 切换器的后端）：写旁车
/// `permission_mode`（重启保持）+ 模式存储缓存（即时生效，运行中可切）。
/// 四档之外取值拒绝（plan / ask / acceptEdits / auto）。切入 Plan 顺带
/// 置位「Plan 模式只读」尾部碎片（提示词分层设计 §4.4——只读围栏不再
/// 靠模型猜），切出移除（尚未投递的也不再带）。
#[tauri::command]
pub async fn session_set_permission_mode(
    session_id: String,
    mode: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let m = crate::permission_mode::PermissionMode::parse(&mode)
        .ok_or_else(|| format!("权限档 {mode:?} 非法（plan / ask / acceptEdits / auto）"))?;
    state
        .permission_modes
        .set_session_mode(&session_id, m)
        .map_err(|e| format!("写入失败: {e}"))?;
    if m == crate::permission_mode::PermissionMode::Plan {
        state
            .fragments
            .set(&session_id, crate::fragments::PLAN_MODE);
    } else {
        state
            .fragments
            .remove(&session_id, crate::fragments::PLAN_MODE);
    }
    Ok(())
}

/// 会话级模型覆盖（v2.58 §3.30 选择器的后端）：写旁车 `model` +
/// `model_provider`（成对，重启保持）+ 覆盖存储缓存（下一轮请求即生效，
/// 运行中可切）。`model` 空 = 清除覆盖（回落设置缺省）。
#[tauri::command]
pub async fn session_set_model(
    session_id: String,
    provider_id: String,
    model: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let m = model.trim().to_string();
    let pair = if m.is_empty() {
        None
    } else {
        let p = provider_id.trim();
        if p.is_empty() {
            return Err("模型覆盖需要档位 id（modelProvider）".into());
        }
        if state.settings.llm_config().profile(p).is_none() {
            return Err(format!("档位不存在: {p}"));
        }
        Some((p, m.as_str()))
    };
    state
        .model_overrides
        .set(&session_id, pair)
        .map_err(|e| format!("写入失败: {e}"))
}

/// 语义档位全集（批 B 推理档）：方言表只映射这些值，表外一律拒收
/// （宁缺勿假——发一个方言映射不了的档位等于没发）。
const THINKING_LEVELS: [&str; 8] = [
    "disabled", "minimal", "low", "medium", "high", "xhigh", "max", "enabled",
];

/// 会话级推理档位（批 B，供应商稿 §5.2）：写旁车 `thinking_level` + 缓存，
/// 下一请求即生效；`level` 空串 = 清除（模型走自家缺省档）。档位在当前
/// 模型梯子里没有对应写法时由解析链 clamp（就近降档/剥除），这里只校验
/// 语义档合法。
#[tauri::command]
pub async fn session_set_thinking(
    session_id: String,
    level: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let l = level.trim().to_string();
    if !l.is_empty() && !THINKING_LEVELS.contains(&l.as_str()) {
        return Err(format!(
            "未知推理档位: {l}（可选 disabled/minimal/low/medium/high/xhigh/max/enabled）"
        ));
    }
    state
        .model_overrides
        .set_thinking(&session_id, if l.is_empty() { None } else { Some(&l) })
        .map_err(|e| format!("写入失败: {e}"))
}

/// 会话删除：忙锁守卫——运行中的会话锁被整轮持有，默认拒绝；开发者开关
/// `developer.allowDeleteRunningSession`（后端直读设置，不信前端转述）
/// 放行时强制删（运行轮余段会重建日志文件）。删除后同步剔除内存条目与
/// 调度"最近聊过"指针（防僵尸注入轮）。幂等：不存在也是 Ok。
#[tauri::command]
pub async fn session_delete(session_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let lock = state
        .sessions
        .lock()
        .expect("sessions poisoned")
        .get(&session_id)
        .cloned();
    if let Some(l) = lock {
        if l.try_lock().is_err() && !state.settings.developer().allow_delete_running_session {
            return Err(
                "会话正在运行，暂不能删除（稍后再试，或在 设置 → 开发者 允许强制删除）".into(),
            );
        }
        state
            .sessions
            .lock()
            .expect("sessions poisoned")
            .remove(&session_id);
    }
    crate::session_log::delete(&state.sessions_dir, &session_id)
        .map_err(|e| format!("删除失败: {e}"))?;
    state.forget_chat(&session_id);
    Ok(())
}

/// 会话正文搜索（标题仍是前端过滤）：UserMessage / AssistantMessage
/// 大小写不敏感子串扫描，mtime 降序；limit 缺省 50、上限 200。
#[tauri::command]
pub async fn session_search(
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<SessionSearchHit>, String> {
    // 批 F-24b / M-18f：这是会话面**唯一** O(全目录) 的同步 IO（逐文件读 jsonl 扫正文），
    // 改前直接跑在 async 线程上 = 卡 IPC 运行时，故包 `spawn_blocking`。
    // 其余会话命令（rename / associate / delete / list / truncate）都是单文件小 IO，
    // 给它们包一层是**无收益的形式主义** ⇒ 不改，口径写进 ROADMAP 边界。
    // 另：审计给的「`session_history` 锁内取快照、锁外投影」实测**不成立**——
    // `Session::events()` 返回借用会话锁守卫的切片，锁外投影只能先 `to_vec()` 深克隆，
    // 比原地投影更贵 ⇒ 那条修法本批不采纳（如实记档，不改口径当修复）。
    let dir = state.sessions_dir.clone();
    tokio::task::spawn_blocking(move || {
        crate::session_log::search(&dir, &query, limit.unwrap_or(50).min(200))
    })
    .await
    .map_err(|e| format!("会话搜索任务被线程池拒绝: {e}"))
}

/// 会话导出为 Markdown 转录（`pick_save_path` 选定路径后调用）：数据 =
/// 会话日志同一投影（内存优先，缺则回读磁盘），与转录/历史重放同源。
/// 返回写出路径。
#[tauri::command]
pub async fn session_export(
    session_id: String,
    path: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // std 锁卫语句先行落变量（match 暂挂临时会横跨 await，非 Send）
    let held = state
        .sessions
        .lock()
        .expect("sessions poisoned")
        .get(&session_id)
        .cloned();
    // 台账 T6：标题**只问这一个会话**——以前这里为了拿一条标题调
    // `session_log::list(整个 sessions/)`，等于每导出一次就把所有会话的日志
    // 逐行扫一遍、旁车逐个读一遍再按 mtime 排序（列表命令的活儿）。
    // 现在旁车优先、缺则用本会话日志里首条用户消息现算，规则与
    // `persist.rs::scan_session_file` 同源（同一个 `SESSION_TITLE_CHARS`）。
    let (events, first_user) = match held {
        Some(lock) => {
            let session = lock.lock().await;
            (
                runner::project_events(session.events(), &session_id),
                first_user_headline(session.events()),
            )
        }
        None => {
            let dir = &state.sessions_dir;
            let log = session_file(dir, &session_id).map_err(|e| format!("非法会话 id: {e}"))?;
            let (session, _) = load_session(&log).map_err(|e| format!("会话日志读取失败: {e}"))?;
            (
                runner::project_events(session.events(), &session_id),
                first_user_headline(session.events()),
            )
        }
    };
    let title = export_title(
        session_store::read_meta(&state.sessions_dir, &session_id).and_then(|m| m.title),
        first_user,
        &session_id,
    );
    let md = crate::export::render_markdown(&title, &session_id, &events);
    std::fs::write(&path, md).map_err(|e| format!("写入失败 {path}: {e}"))?;
    Ok(path)
}

/// 导出的标题三层优先：旁车重命名标题 → 本会话首条用户消息截断 → 会话 id。
/// 与列表层（`persist.rs::list_sessions`）同一条次序，差别只在"只算这一个会话"。
/// 抽成纯函数是为了能被用例钉住——命令本体要 Tauri `State`，测试打不进去。
pub(crate) fn export_title(
    sidecar: Option<String>,
    first_user: Option<String>,
    session_id: &str,
) -> String {
    sidecar
        .filter(|t| !t.trim().is_empty())
        .or(first_user)
        .unwrap_or_else(|| session_id.to_string())
}

/// 列表层自动标题的本会话版本：首条用户消息前 `SESSION_TITLE_CHARS` 字。
/// 宽度常量从 session_store 取，**不在这里重抄 40**（同一读数两个出处必漂）。
pub(crate) fn first_user_headline(events: &[SessionEvent]) -> Option<String> {
    events.iter().find_map(|e| match e {
        SessionEvent::UserMessage { content, .. } => Some(
            content
                .chars()
                .take(session_store::SESSION_TITLE_CHARS)
                .collect(),
        ),
        _ => None,
    })
}

/// 消息编辑重发的截断步（v2.57 §3.34）：把日志截到 `pos` 之前（rewind_to
/// + fork，⑨ 先例）并物理同步镜像（persist rewrite，`.undo.bak` 留痕同
/// ⑲）。`pos = None` = 最后一条用户消息（本进程实时发送的消息无事件位，
/// 由后端解析）。忙锁 try_lock 守卫：运行中拒绝（前端编辑按钮同规禁用）；
/// 重发由前端走既有 `agent_chat`（审批/用量/提取零新链路）。
#[tauri::command]
pub async fn session_truncate_after(
    session_id: String,
    pos: Option<usize>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let held = state
        .sessions
        .lock()
        .expect("sessions poisoned")
        .get(&session_id)
        .cloned()
        .ok_or_else(|| "会话不存在（还没有任何消息）".to_string())?;
    if held.try_lock().is_err() {
        return Err("会话正在运行，暂不能编辑重发（等这轮结束再试）".into());
    }
    let mut session = held.lock().await;
    crate::session_log::truncate_after(&mut session, pos)?;
    crate::session_log::rewrite_after_undo(&state.sessions_dir, &session_id, &session);
    Ok(())
}

/// 崩溃恢复裁决（G2，可靠性稿 §5）：用户对 uncertain 悬空调用给出事实
/// 认定（视为已完成 / 视为未执行 / 检查后记录），落 `ToolExecutionUncertain`
/// 事件（只写一次，重放/重启不再询问）并实时回推投影（UncertainResolved
/// 收束决策卡 + 合成 ToolResult 按裁决闭合工具行）。忙锁 try_lock 同
/// 编辑重发：运行中拒绝——裁决改写下一轮的请求形状，与进行中的轮互斥。
/// 日志不回填 ToolResult：悬空是日志事实，闭合是投影层的合成。
#[tauri::command]
pub async fn session_resolve_uncertain(
    session_id: String,
    call_id: String,
    decision: String,
    note: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let held = state
        .sessions
        .lock()
        .expect("sessions poisoned")
        .get(&session_id)
        .cloned()
        .ok_or_else(|| "会话不存在（还没有任何消息）".to_string())?;
    if held.try_lock().is_err() {
        return Err("会话正在运行，暂不能裁决（等这轮结束再试）".into());
    }
    let dec =
        UncertainDecision::parse(&decision).ok_or_else(|| format!("未知裁决类型: {decision}"))?;
    let note = note.unwrap_or_default();
    if matches!(dec, UncertainDecision::Inspected) && note.trim().is_empty() {
        return Err("「检查后记录」需要填写检查结果".into());
    }
    let mut session = held.lock().await;
    let scan = session_store::scan_uncertain(session.events());
    if !scan.dangling.contains(call_id.as_str()) {
        return Err("该调用已有结果，不是待裁决的悬空调用".into());
    }
    if scan.decisions.contains_key(call_id.as_str()) {
        return Err("该调用已经裁决过了".into());
    }
    let ts = now_ms();
    let seq = session.append(SessionEvent::ToolExecutionUncertain {
        call_id: plugin_api::CallId(call_id.clone()),
        decision: dec,
        note: note.trim().to_string(),
        ts,
    });
    crate::session_log::append_slice(&state.sessions_dir, &session_id, &session, seq);
    // 实时回推（与重放同形）：卡收束 + 行闭合。tool 从 ToolCall 事件
    // 找回；声明位缺失回落 call_id（ApprovalResolved 同款）。
    let tool = session
        .events()
        .iter()
        .find_map(|e| match e {
            SessionEvent::ToolCall {
                call_id: c, tool, ..
            } if c.0 == call_id => Some(tool.clone()),
            _ => None,
        })
        .unwrap_or_else(|| call_id.clone());
    state.sink.emit(&AgentEvent::UncertainResolved {
        session_id: session_id.clone(),
        call_id: call_id.clone(),
        tool: tool.clone(),
        decision: dec.as_str().into(),
        note: note.trim().to_string(),
    });
    let (output, error) = session_store::closure(Some((dec, note.trim())));
    state.sink.emit(&AgentEvent::ToolResult {
        session_id,
        call_id,
        ok: true,
        output,
        error,
    });
    Ok(())
}

/// 子 Agent 转录读取（§3.16，v2.52）：`sub-transcripts/<child_id>.jsonl`
/// load 后走与 `session_history` **同一** `project_events` 全量投影——
/// 前端页签的数据形状与主转录一致。文件缺席 = Err（页签如实报不可用）。
#[tauri::command]
pub async fn subagent_transcript(
    child_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AgentEvent>, String> {
    let log = session_file(&state.sub_transcripts_dir, &child_id)
        .map_err(|e| format!("非法子会话 id: {e}"))?;
    if !log.exists() {
        return Err(format!("子转录不存在（未落盘或已清理）: {child_id}"));
    }
    let (session, skipped) = load_session(&log).map_err(|e| format!("子转录读取失败: {e}"))?;
    if skipped > 0 {
        tracing::warn!(child = %child_id, skipped, "sub transcript 坏行跳过");
    }
    Ok(runner::project_events(session.events(), &child_id))
}

/// 跨平台打开目录（reveal_in_explorer / memory_reveal_folder 共用壳）。
pub fn open_in_explorer(p: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(p)
            .spawn()
            .map_err(|e| format!("打开文件管理器失败: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(p)
            .spawn()
            .map_err(|e| format!("打开文件管理器失败: {e}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(p)
            .spawn()
            .map_err(|e| format!("打开文件管理器失败: {e}"))?;
    }
    Ok(())
}

/// 在系统文件管理器中打开项目目录（§2.3 v3.9 项目菜单「在文件夹中显示」）：
/// 路径必须是当前 fsRoots 之一（后端直读设置校验，不信前端转述——与
/// `session_associate` 同一防线），不给任意路径开执行面。
#[tauri::command]
pub async fn reveal_in_explorer(path: String, state: State<'_, AppState>) -> Result<(), String> {
    let p = path.trim();
    let roots = state.settings.fs_roots();
    if !roots.iter().any(|r| r == p) {
        return Err(format!("「{p}」不在 设置 → 工具 → 工作区目录 里，拒绝打开"));
    }
    open_in_explorer(p)
}

/// 打开外部网页链接（批 A「如何获取 API Key ↗」= 预设供应商文档）：只放
/// 行 https、无空白/控制字符、限长——不给任意协议/命令注入开执行面；
/// shell 套路同 `open_in_explorer`（explorer / open / xdg-open）。
#[tauri::command]
pub async fn open_external_url(url: String) -> Result<(), String> {
    let u = url.trim();
    if !u.starts_with("https://") {
        return Err("只允许打开 https 链接".into());
    }
    if u.len() > 500 || u.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err("链接形状不合法，拒绝打开".into());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(u)
            .spawn()
            .map_err(|e| format!("打开链接失败: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(u)
            .spawn()
            .map_err(|e| format!("打开链接失败: {e}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(u)
            .spawn()
            .map_err(|e| format!("打开链接失败: {e}"))?;
    }
    Ok(())
}
