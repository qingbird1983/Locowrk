//! `/压缩` 的后端那一半（S2 刀⑦ · AC-S2-11）：把主层深度压缩从"只有到线才自动动"
//! 变成"人也能当场要一次"。薄适配层（AGENTS §3）——判定、摘要调用、验货、落账全在
//! `compact` 侧，这一片只做三件事：凑那把尺、拿会话锁、把结果分两路交出去。
//!
//! **为什么压成的负载走事件、返回值只说"这一趟没动"**：AGENTS §3 那条"流式输出走事件
//! 推送、不塞进 command 返回值"，而压缩落账本来就是一条 `ContextCompacted` 会话事件。
//! 实时推它时复用 [`crate::runner::projection::project_events`] 那一份构造（只喂落账
//! 那**一条**，不重放整个会话），于是"断层条长什么样"在界面上只有一个出处，手动压的
//! 与自动压的**同形**——包括 [查看详情] 里的摘要预览与 [撤销]。
//!
//! **为什么这把尺不从这儿另算**：`/压缩` 要的阈值与窗口跟跑一轮要的是同一份，抄第二遍
//! 就是两个出处（改了滑杆而手动压缩照旧按老数走 = v2.138「看到的 = 触发用的」那条纪律
//! 的反面）。所以调 [`crate::commands::chat::usage_policy`]——刀⑦ 顺手把那段从
//! `agent_chat` 里提出来，就是为了这里不重写。
//!
//! S4 刀③b/③c：`organize_now`（`/整理` 那条链）与其开发者面板入口随记忆树整线
//! 删除，本片不再有「自动整理熔断复位」这类相邻职责——只剩压缩一件事。

use tauri::State;

use session_store::SessionEvent;

use crate::compact::deep::Outcome;
use crate::runner::projection;
use crate::state::AppState;

/// 一次手动压缩的结果。`compacted` = 有没有压成；`reason` = 没压成时的**判据名**。
///
/// 这里**不拼人话**：文案住语言包（同类只许一种形态），前端按名取 i18n 那条。
/// `reason` 的取值域 = [`crate::compact::plan`] 的几条（`unknown-window` /
/// `below-line` / `fused` / `too-few-rounds` / `nothing-to-summarize`）+ 失败那三条
/// （`summary-call` / `overflow` / `rejected`）+ 手动通路理论上不会来的 `off`。
/// 手动带 `manual` 让开了前两条"线"，所以界面上正常**看不到** `below-line` 与
/// `fused`——真看到就是判据写错了，那也照样如实显示，不粉饰成"已压缩"。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompactNow {
    pub compacted: bool,
    pub reason: Option<&'static str>,
}

/// 手动通路理论上到不了这一格（`run_manual` 不看那枚开关），但 `Outcome::Off` 是
/// 现成的变体，映射表里得有它——名字同样只许一个出处（与 `deep::WHY_*` 同一条纪律，
/// 判据 `SS24` 拿它跟前端 `COMPACT_SKIP_KEYS` 对账）。
const REASON_OFF: &str = "off";

fn not_compacted(reason: &'static str) -> CompactNow {
    CompactNow {
        compacted: false,
        reason: Some(reason),
    }
}

/// `/压缩`：当场跑一次主层深度压缩（不受触发线与熔断限制，理由见
/// [`crate::compact::deep::Compactor::run_manual`]）。会话锁整趟持有——压的时候
/// 不许另一条请求同时改这份窗口，那正是"压完还是超窗"的来路。
#[tauri::command]
pub async fn compact_now(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<CompactNow, String> {
    let policy = crate::commands::chat::usage_policy(&state, &session_id).await;
    let session = state.session_lock(&session_id);
    let mut session = session.lock().await;
    let outcome = state
        .compactor
        .run_manual(&mut session, &session_id, &policy)
        .await;
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
        Outcome::Off => not_compacted(REASON_OFF),
        Outcome::Skipped(reason) => not_compacted(reason),
        Outcome::Failed { why, .. } => not_compacted(why),
    })
}
