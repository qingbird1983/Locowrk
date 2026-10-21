//! `/压缩` 的后端那一半（specs/13 · C3 / AC-7）：把上下文压缩从"只有到线才自动动"
//! 变成"人也能当场要一次"。薄适配层（AGENTS §3）——判定、请求装配、摘要调用、
//! 验货、落账全在 `compact` 侧，这一片只做三件事：凑那把尺、拿会话锁、把结果
//! 分两路交出去。
//!
//! **手动那一下不受"自动压缩关着"影响**（上游同款）：那枚开关关的是"这一层
//! **自己**动不动手"，人主动按的这一下不该被它挡（同设置页那句「手动 `/压缩`
//! 不受此限」在压缩层的同一条姿态，也是可配置原则②的反面：开关不许长成禁止）。
//!
//! **为什么压成的负载走事件、返回值只说"这一趟没成"**：AGENTS §3 那条"流式输出走
//! 事件推送、不塞进 command 返回值"，而压缩落账本来就是一条 `ContextCompacted`
//! 会话事件。实时推它时复用 [`crate::runner::projection::project_events`] 那一份
//! 构造（只喂落账那**一条**，不重放整个会话），于是"断层条长什么样"在界面上只有
//! 一个出处，手动压的与自动压的**同形**。
//!
//! **失败判据名与自动那一路共用同一张表**（[`crate::compact::outcome::REASONS`]）：
//! 手动也会撞「没内容可摘」那几档 ⇒ 不为手动单开一套名字。

use tauri::State;

use session_store::SessionEvent;

use crate::compact::Outcome;
use crate::runner::projection;
use crate::state::AppState;

/// 一次手动压缩的结果。`compacted` = 有没有压成；`reason` = 没压成时的**判据名**。
///
/// 这里**不拼人话**：文案住语言包（同类只许一种形态），前端按名取 i18n 那条。
/// `reason` 的取值域 = [`crate::compact::outcome::REASONS`]（9 条）——
/// **跳过不进这张表**（不产事件 ⇒ 界面上没有那一档文案）。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompactNow {
    pub compacted: bool,
    pub reason: Option<&'static str>,
}

/// `/压缩`：当场跑一次压缩。**自动压缩关着也照样能压**（上游同款）。会话锁整趟
/// 持有——压的时候不许另一条请求同时改这份窗口，那正是"压完还是超窗"的来路。
#[tauri::command]
pub async fn compact_now(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<CompactNow, String> {
    let policy = crate::commands::chat::usage_policy(&state, &session_id).await;
    let session = state.session_lock(&session_id);
    let mut session = session.lock().await;
    // 手动那一下**不发**「开始了」：界面自己就落了「正在压缩」那条线（前端在发
    // 命令**之前**落的，命令返回后改写）——后端再发一次就是两根同义的线。
    // 摘要正文的流式增量照发（上游 `Compaction.Delta` 不分手动自动）。
    let on_start = || {};
    let on_delta = |delta: &str| {
        state.sink.emit(&crate::events::AgentEvent::CompactStatus {
            session_id: session_id.clone(),
            phase: crate::events::compact_phase::DELTA.to_string(),
            reason: None,
            text: Some(delta.to_string()),
        });
    };
    let hooks = crate::compact::deep::Hooks {
        on_start: &on_start,
        on_delta: &on_delta,
    };
    // 跑 + 落盘（**一个调用**）：手动这一趟不在任何一轮里，没有 runner 那份
    // 落盘游标可借 ⇒ 落盘口在压缩层这一侧（`run_manual_and_persist`）。
    //
    // 漏了落盘是什么样：压完看着一切正常（断层条、摘要、界面全对），但磁盘上
    // 还是压前那一份历史 ⇒ 重开应用，压缩连同摘要一起消失，那轮 LLM 白花。
    // 自动那条在 `runner` 里落（step 边界那一段），手动这条以前谁也没替它落。
    let outcome = crate::compact::deep::run_manual_and_persist(
        &state.compactor,
        &mut session,
        &session_id,
        Some(state.sessions_dir.as_path()),
        &policy,
        &hooks,
    )
    .await;
    // 判据名**原样透传**（不另敲一个字面量）：`Outcome::reason` 是那一族的唯一取数口。
    let reason = outcome.reason();
    Ok(match outcome {
        Outcome::Compacted { .. } => {
            // 落账那一条就在末尾（`run_manual` 里刚 append 的）；只投影它一条。
            let seam = session
                .events()
                .iter()
                .rev()
                .find(|e| matches!(e, SessionEvent::ContextCompacted { .. }));
            if let Some(ev) = seam {
                for out in projection::project_events(std::slice::from_ref(ev), &session_id) {
                    state.sink.emit(&out);
                }
            }
            CompactNow {
                compacted: true,
                reason: None,
            }
        }
        // 失败：判据名**原样透传**（不另敲一个字面量）。
        Outcome::Failed { .. } => CompactNow {
            compacted: false,
            reason,
        },
        // 跳过 / 熔断：手动那一格理论上到不了（手动不受开关与熔断影响），
        // 到了也如实说"没压成"，但**不带判据名**（跳过不进界面文案表）。
        _ => CompactNow {
            compacted: false,
            reason: None,
        },
    })
}
