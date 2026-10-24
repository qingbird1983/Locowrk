//! 会话元数据的写侧（旁车字段与内存缓存两处一起写）：重命名 / 归属 / 权限档 /
//! 模型覆盖 / 推理档位。共同形状 = 校验参数 → 写旁车（重启保持）→ 换入缓存
//! （下一轮请求即生效，运行中可切）。fsRoots 白名单校验 `ensure_in_fs_roots`
//! 被 `agent_chat` 的归属先行（㉘·A·fix）与 `reveal_in_explorer`（T28 并处）共用，
//! 所以本片带一个 `pub(crate)` 出口。

use tauri::State;

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

/// fsRoots 白名单校验（后端直读设置，不信前端转述）：目标必须在当前 fsRoots
/// 里。`session_associate` / `agent_chat` 的归属先行（㉘·A·fix）/
/// `reveal_in_explorer` 三处共用一条防线（T28 并处）；拒答尾巴**刻意**随动作
/// 走（「不能关联」/「拒绝打开」），由 `tests_session_log::sessions_texts`
/// 夹具逐字钉住。
pub(crate) fn ensure_in_fs_roots(roots: &[String], target: &str, deny: &str) -> Result<(), String> {
    if roots.iter().any(|r| r == target) {
        Ok(())
    } else {
        Err(format!(
            "「{target}」不在 设置 → 工具 → 工作区目录 里，{deny}"
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
        ensure_in_fs_roots(&state.settings.fs_roots(), ws, "不能关联")?;
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
