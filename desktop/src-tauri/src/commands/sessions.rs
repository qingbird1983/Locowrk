//! 会话运营命令（v2.49，UI shell 文档 §3.6 行尾 `…` 菜单的后端）：重命名
//! / 删除 / 搜索 / 导出。§3.16 子转录读取（v2.52）。薄适配层——文件面在
//! session_store::persist（经 session_log 薄封装），投影与渲染在 runner /
//! export，这里只接参数。

use tauri::State;

use session_store::{load_session, session_file, SessionSearchHit};

use crate::events::AgentEvent;
use crate::runner;
use crate::state::AppState;

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
/// 四档之外取值拒绝（plan / ask / acceptEdits / auto）。
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
        .map_err(|e| format!("写入失败: {e}"))
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
    Ok(crate::session_log::search(
        &state.sessions_dir,
        &query,
        limit.unwrap_or(50).min(200),
    ))
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
    let events = match held {
        Some(lock) => {
            let session = lock.lock().await;
            crate::runner::project_events(session.events(), &session_id)
        }
        None => {
            let dir = &state.sessions_dir;
            let log = session_store::session_file(dir, &session_id)
                .map_err(|e| format!("非法会话 id: {e}"))?;
            let (session, _) =
                session_store::load_session(&log).map_err(|e| format!("会话日志读取失败: {e}"))?;
            crate::runner::project_events(session.events(), &session_id)
        }
    };
    let title = crate::session_log::list(&state.sessions_dir)
        .into_iter()
        .find(|m| m.id == session_id)
        .and_then(|m| m.title)
        .unwrap_or_else(|| session_id.clone());
    let md = crate::export::render_markdown(&title, &session_id, &events);
    std::fs::write(&path, md).map_err(|e| format!("写入失败 {path}: {e}"))?;
    Ok(path)
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
