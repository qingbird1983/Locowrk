//! 压成纯文本那一档的转录形态（上游 `packages/core/src/session/compaction.ts:486-526`
//! `flattenAndDropOldest` + `:775-795` `flattenMessage`）。
//!
//! 形态：**一段历史要改形状时，改成文本发**（`[previous checkpoint][转录]`），
//! 而不是编辑过的消息列。改消息列会丢掉"这次到底丢了什么"的可读性，而模型
//! 看到一段纯文本时反而更清楚地知道"这是转录，不是当下的指令"。
//!
//! 丢最老的规则同上游：一次往返（一条 user 消息 + 它之后的回复）**整组**留或
//! 整组丢；丢的时候在最前面留一句 `[N older exchange(s) omitted]`。

use kernel_agent::LlmRequest;
use session_store::Message;

use super::deliver::estimate_text;

/// 一条 user 消息 + 它之后的回复 = 一次往返（上游 `compaction.ts:503-512`）。
///
/// 分组口径：遇到 user 消息开新组，其余消息并进当前组（组内用 `\n` 连）。
pub(crate) fn exchanges(messages: &[Message]) -> Vec<String> {
    let mut groups: Vec<String> = Vec::new();
    for m in messages {
        let text = session_store::message_to_text(m);
        if text.is_empty() {
            continue;
        }
        let starts = matches!(m, Message::User { .. }) || groups.is_empty();
        if starts {
            groups.push(text);
        } else if let Some(last) = groups.last_mut() {
            last.push('\n');
            last.push_str(&text);
        }
    }
    groups
}

/// 丢了几次往返的那一句（上游 `compaction.ts:514-515` 逐字）。
///
/// 单数 / 复数两种形态都要有：上游是 `omitted === 1 ? "exchange" : "exchanges"`。
pub(crate) fn omitted_note(omitted: usize) -> String {
    if omitted == 0 {
        return String::new();
    }
    let word = if omitted == 1 {
        "exchange"
    } else {
        "exchanges"
    };
    format!("[{omitted} older {word} omitted]\n\n")
}

/// 把请求改写成"文本形态"并丢掉装不下的最旧几组（上游 `flattenAndDropOldest`）。
///
/// `lead_len` = 前面那段**原样保留**的条数（上游的 `lead`：上一个 checkpoint）。
/// `room` = 留给转录的额度（调用方算好：目标大小 − lead 的估算 − 那句省略标记）。
///
/// `None` = 连最新一次往返都装不下（上游同款：那就没什么可发的了）。
pub(crate) fn flatten_and_drop_oldest(
    request: &LlmRequest,
    lead_len: usize,
    room: u64,
) -> Option<LlmRequest> {
    let lead = &request.messages[..lead_len.min(request.messages.len())];
    let groups = exchanges(&request.messages[lead.len()..]);
    if groups.is_empty() {
        return None;
    }
    let kept = keep_newest(&groups, room);
    if kept.is_empty() {
        return None;
    }
    let omitted = groups.len() - kept.len();
    let body = format!("{}{}", omitted_note(omitted), kept.join("\n\n"));
    let mut messages = lead.to_vec();
    messages.push(Message::user(body));
    Some(LlmRequest {
        messages,
        ..request.clone()
    })
}

/// 从最新往回装，装得下的那几组（最旧的先让位）。
fn keep_newest(groups: &[String], room: u64) -> Vec<String> {
    let mut kept: Vec<String> = Vec::new();
    let mut total = 0u64;
    for g in groups.iter().rev() {
        let size = estimate_text(g) + 1;
        if total.saturating_add(size) > room {
            break;
        }
        total += size;
        kept.push(g.clone());
    }
    kept.reverse();
    kept
}
