//! ask_user 选择卡命令（v4.11 §3.39①）：应答 / 续期 / 补拉三件，全部
//! 薄适配——业务在 `crate::ask::AskHub`。应答载荷用 tagged enum 一条命令
//! 承载两路（选择 / 跳过），避免 skip 语义分裂成两个端点。

use tauri::State;

use crate::ask::AskAnswer;
use crate::events::AgentEvent;
use crate::state::AppState;

/// `ask_respond` 请求体：`{ kind: "answer", labels: [...], note? }` 或
/// `{ kind: "skip", reason? }`（用户主动跳过不带 reason——工具结果只落
/// `{skipped:true}`；超时/停止的自动跳过才带服务端生成的 reason）。
#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AskAnswerDto {
    Answer {
        labels: Vec<String>,
        #[serde(default)]
        note: String,
    },
    Skip {
        #[serde(default)]
        reason: Option<String>,
    },
}

impl From<AskAnswerDto> for AskAnswer {
    fn from(dto: AskAnswerDto) -> Self {
        match dto {
            AskAnswerDto::Answer { labels, note } => AskAnswer::Answer { labels, note },
            AskAnswerDto::Skip { reason } => AskAnswer::Skipped { reason },
        }
    }
}

/// 回填应答/跳过并唤醒挂起的 `ask_user` 工具调用。请求不存在（已处理 /
/// 守夜已超时跳过）= Err，前端据此摘卡。
#[tauri::command]
pub async fn ask_respond(
    request_id: String,
    answer: AskAnswerDto,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.ask.respond(&request_id, answer.into())
}

/// 卡内首次交互续期：摘除本次请求的守夜（打字不被超时吞）。请求已收束
/// = 幂等空转。
#[tauri::command]
pub async fn ask_snooze(request_id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.ask.snooze(&request_id);
    Ok(())
}

/// 补拉未决请求（前端挂载 / 重连 / 刷新时）：返回与实时通道
/// `agent://ask-user` 同形的 AskUser 事件，前端同一消费口入列。
#[tauri::command]
pub async fn list_pending_asks(state: State<'_, AppState>) -> Result<Vec<AgentEvent>, String> {
    Ok(state.ask.pending())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn answer_dto_parses_with_default_note() {
        let dto: AskAnswerDto =
            serde_json::from_value(serde_json::json!({ "kind": "answer", "labels": ["A"] }))
                .expect("answer dto");
        match AskAnswer::from(dto) {
            AskAnswer::Answer { labels, note } => {
                assert_eq!(labels, vec!["A".to_string()]);
                assert_eq!(note, "");
            }
            _ => panic!("expected Answer"),
        }
    }

    #[test]
    fn skip_dto_parses_without_reason() {
        let dto: AskAnswerDto =
            serde_json::from_value(serde_json::json!({ "kind": "skip" })).expect("skip dto");
        match AskAnswer::from(dto) {
            AskAnswer::Skipped { reason } => assert_eq!(reason, None),
            _ => panic!("expected Skipped"),
        }
    }
}
