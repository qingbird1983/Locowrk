//! 批 F-23（审计 M-24b）：`pending_slot` 由「反向全扫」改「命中即停」的等价证明。
//!
//! 病根 = 旧写法在 call_id 没有占位时（补偿路径的 `{id}-undo` **每条**都是）一路扫到
//! 日志头，`apply` / `state_at` / `load_session` 因此对长会话退化到 O(n²)。改法 = 只看
//! 反向遇到的**第一条** assistant。这条改动的正确性不靠"看起来一样"，靠下面两段：
//! ① 全部**可达**形状逐事件等值；② 唯一会让两者分岔的形状（晚到的 ToolResult 跨过一条
//!    更新的 assistant）现网不可达，把新行为钉死——将来内核若真改成并行执行工具或延迟
//!    落结果，这条测试会先红，而不是等用户看见丢失的工具名。

use super::*;

use crate::{Message, SessionState, ToolCallMsg};

/// **被删原文**当参考实现：HEAD 版 `SessionState::pending_slot` 逐字抄来（只把 `self`
/// 换成参数）——反向遇到 assistant 就在它里面找，**找不到继续往前扫**。
fn ref_pending_slot_old<'a>(
    state: &'a mut SessionState,
    call_id: &CallId,
) -> Option<&'a mut ToolCallMsg> {
    state.messages.iter_mut().rev().find_map(|m| match m {
        Message::Assistant { tool_calls, .. } => tool_calls.iter_mut().find(|c| &c.id == call_id),
        _ => None,
    })
}

/// 参考折叠：只有碰 `pending_slot` 的两个分支走旧扫描，其余事件**委托生产 `apply`**。
/// 不把整份状态机抄一遍——抄出来的副本自己也会漂，那时"两边相等"就不再证明任何事。
fn apply_ref(state: &mut SessionState, e: &SessionEvent) {
    match e {
        SessionEvent::ToolCall {
            call_id,
            tool,
            args,
            ..
        } => {
            if let Some(slot) = ref_pending_slot_old(state, call_id) {
                slot.tool = tool.clone();
                slot.args = args.clone();
            }
        }
        SessionEvent::ToolResult {
            call_id,
            output,
            error,
        } => {
            let (tool, args) = ref_pending_slot_old(state, call_id)
                .map(|s| (s.tool.clone(), s.args.clone()))
                .unwrap_or_else(|| (String::new(), json!(null)));
            state.messages.push(Message::ToolResult {
                call_id: call_id.clone(),
                tool,
                args,
                output: output.clone(),
                error: error.clone(),
            });
        }
        other => state.apply(other),
    }
}

fn assistant_with(content: &str, calls: &[&str]) -> SessionEvent {
    SessionEvent::AssistantMessage {
        content: content.into(),
        tool_calls: calls.iter().map(|c| CallId((*c).into())).collect(),
    }
}

fn call_of(id: &str, tool: &str) -> SessionEvent {
    SessionEvent::ToolCall {
        call_id: CallId(id.into()),
        tool: tool.into(),
        args: json!({ "id": id }),
        plugin_id: None,
        plugin_version: Version::new(0, 1, 0),
        side_effect: SideEffect::Pure,
    }
}

fn result_of(id: &str, err: Option<&str>) -> SessionEvent {
    SessionEvent::ToolResult {
        call_id: CallId(id.into()),
        output: json!({ "done": id }),
        error: err.map(str::to_string),
    }
}

#[test]
fn pending_slot_stops_at_nearest_assistant() {
    // ① 可达形状：内核的追加次序恒为「一条 assistant → 它自己的 tool_call /
    //    tool_result（工具串行 await，kernel.rs:257/:310/:335）→ 下一 step 的
    //    assistant」，而压缩落在**本轮 assistant 之前**（kernel.rs:232 注释即此），
    //    因此压缩不可能插进"声明"与"结果"之间；补偿的 `{id}-undo` 从未被任何
    //    assistant 声明过（compensation.rs:101 现造新 id）。
    let reachable: Vec<SessionEvent> = vec![
        user("帮我读文件"),
        assistant_with("我来读", &["c1"]),
        call_of("c1", "fs_read"),
        result_of("c1", None),
        assistant_with("再写两处", &["c2", "c3"]),
        call_of("c2", "fs_write"),
        result_of("c2", Some("权限不足")),
        call_of("c3", "shell"),
        result_of("c3", None),
        assistant_with("", &[]), // 纯文本答复：占位表为空的 assistant
        user("撤销刚才那步"),
        result_of("c3-undo", None),  // 补偿：无占位 ⇒ 两种写法都 None
        compacted(4, 5, "早期摘要"), // step 之间的压缩（保留区 = 最后 4 条 ⇒ 起点 5）
        assistant_with("压缩后继续", &["c4"]),
        call_of("c4", "echo"),
        result_of("c4", None),
        result_of("c9-never-declared", None), // 悬空结果，最近 assistant 是上面那条
        assistant_with("收尾", &[]),
        user("完事"),
    ];
    let mut real = SessionState::default();
    let mut reference = SessionState::default();
    // 阳性对照要在**过程中**记，不能在末尾看：序列里那条 `compacted(4, …)` 会把
    // 早期消息（含 fs_write 的结果）折出状态，末尾断言因此恒假（首跑就红在这里）。
    let mut saw_backfilled_result = false;
    let mut saw_filled_slot = false;
    for (i, e) in reachable.iter().enumerate() {
        real.apply(e);
        apply_ref(&mut reference, e);
        assert_eq!(
            real.messages, reference.messages,
            "可达形状第 {i} 个事件后投影分岔 ⇒ 新旧 pending_slot 不再等价"
        );
        saw_backfilled_result |= real
            .messages
            .iter()
            .any(|m| matches!(m, Message::ToolResult { tool, .. } if tool == "fs_write"));
        saw_filled_slot |= real.messages.iter().any(|m| {
            matches!(m, Message::Assistant { tool_calls, .. }
                if tool_calls.iter().any(|c| c.id.0 == "c1" && c.tool == "fs_read"))
        });
    }
    // 阳性对照：这段序列真的走到了两条回填路径（ToolCall 填占位 + ToolResult 抄工具名），
    // 否则"两边都空所以相等"是零证明力的假绿。
    assert!(saw_filled_slot, "ToolCall 没填进占位 ⇒ 等值断言没有证明力");
    assert!(
        saw_backfilled_result,
        "ToolResult 没抄到工具名 ⇒ 等值断言没有证明力"
    );

    // ② 不可达形状（唯一分岔点）：晚到的 ToolResult 跨过一条**更新的** assistant 去
    //    回填更早的占位。旧写法扫得到、新写法在最近一条就停 ⇒ 分岔是**设计**，钉死。
    let out_of_order: Vec<SessionEvent> = vec![
        assistant_with("早", &["cA"]),
        call_of("cA", "fs_read"),
        assistant_with("晚", &["cB"]), // cA 的结果还没落，新 step 的 assistant 先到
        result_of("cA", None),
    ];
    let mut real = SessionState::default();
    let mut reference = SessionState::default();
    for e in &out_of_order {
        real.apply(e);
        apply_ref(&mut reference, e);
    }
    fn last_result(s: &SessionState) -> (String, serde_json::Value) {
        match s.messages.last().expect("有结果") {
            Message::ToolResult { tool, args, .. } => (tool.clone(), args.clone()),
            m => panic!("末条不是 ToolResult：{m:?}"),
        }
    }
    assert_eq!(
        last_result(&real),
        (String::new(), json!(null)),
        "新行为 = 最近 assistant 里没有 cA ⇒ 工具名与参数留空"
    );
    assert_eq!(
        last_result(&reference),
        ("fs_read".to_string(), json!({ "id": "cA" })),
        "旧行为 = 一路扫回更早的 assistant ⇒ 回填成功"
    );
    assert_ne!(
        real.messages, reference.messages,
        "两种写法在这个形状上必须分岔（否则②这颗钉子是空的）"
    );
}
