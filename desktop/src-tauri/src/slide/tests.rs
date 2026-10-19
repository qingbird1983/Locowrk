//! 刀⑤ 判据：滑窗保护网"只在熔断后开、丢过程留提问、不拆 assistant 与回执的对"
//! （specs/09 AC-S2-8）。
//!
//! 沿用微压缩那件贯穿全文件的取数纪律：**主层阈值不钉魔法数**，一律从
//! `estimate_tokens(&msgs)` 推出来（[`over_line`] / [`under_line`]）。钉死一颗数的后果
//! 那边已经实测过一次：夹具体量一调整，用例就从"断言被丢了"退化成"断言什么都没触发"，
//! 全绿而生产里这一层根本没跑。

use plugin_api::CallId;
use serde_json::json;
use session_store::{summarized_message, SUMMARIZED_MARKER};

use super::*;
use crate::settings::context::SLIDE_KEEP_RUNS_DEFAULT;

fn plan_of(messages: &[Message], main: Option<u64>, fused: bool) -> SlidePlan {
    plan(&SlideInput {
        messages,
        main_threshold: main,
        fused,
        // 刀⑥：段数从常量改成设置项，夹具给缺省那颗 1 ⇒ 本文件所有断言一字未改
        // 仍然绿，这就是"缺省 = 改前行为"的实证（AC-S2-10 那条等价性要求）。
        keep_runs: crate::settings::context::SLIDE_KEEP_RUNS_DEFAULT as usize,
    })
}

/// 按下标切完之后，**保留集里**那些没有前置声明的工具回执（应当恒为空）。
/// 这是本文件最要紧的一把尺：它不问判据"打算丢什么"，只问切完发出去的那份东西
/// 供应商收不收——拆对这一档只有从结果面才看得出来。
fn orphan_results(messages: &[Message], dropped: &[usize]) -> Vec<String> {
    let gone: HashSet<usize> = dropped.iter().copied().collect();
    let declared: HashSet<String> = messages
        .iter()
        .enumerate()
        .filter(|(i, _)| !gone.contains(i))
        .filter_map(|(_, m)| match m {
            Message::Assistant { tool_calls, .. } => Some(
                tool_calls
                    .iter()
                    .map(|c| c.id.0.clone())
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect();
    messages
        .iter()
        .enumerate()
        .filter(|(i, _)| !gone.contains(i))
        .filter_map(|(_, m)| match m {
            Message::ToolResult { call_id, .. } if !declared.contains(&call_id.0) => {
                Some(call_id.0.clone())
            }
            _ => None,
        })
        .collect()
}

/// 保留下来的消息里，提问按顺序的正文。
fn questions(messages: &[Message], dropped: &[usize]) -> Vec<String> {
    let gone: HashSet<usize> = dropped.iter().copied().collect();
    messages
        .iter()
        .enumerate()
        .filter(|(i, _)| !gone.contains(i))
        .filter_map(|(_, m)| match m {
            Message::User { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect()
}

/// 保留下来的工具回执的 call_id（"过程一条不留"就从这里数）。
fn kept_calls(messages: &[Message], dropped: &[usize]) -> Vec<String> {
    let gone: HashSet<usize> = dropped.iter().copied().collect();
    messages
        .iter()
        .enumerate()
        .filter(|(i, _)| !gone.contains(i))
        .filter_map(|(_, m)| match m {
            Message::ToolResult { call_id, .. } => Some(call_id.0.clone()),
            _ => None,
        })
        .collect()
}

// ---------- 熔断未开 ⇒ 零动作（AC-S2-8 第二句）----------

/// 三个"即使"各钉一种漂法：即使窗口已知、即使用量早已到线、即使历史长得能丢。
/// 保护网悄悄吃掉正常会话的过程，是这一层最坏的失效形状（模型会突然"忘记"自己
/// 刚做过什么，而界面上没有任何一行解释为什么）。
#[test]
fn an_unfused_session_sees_zero_action() {
    let msgs = rounds(8, 4_000);
    for main in [over_line(&msgs), Some(1), under_line(&msgs)] {
        let p = plan_of(&msgs, main, false);
        assert_eq!(p.reason, "not-fused", "熔断没开就该报这个数");
        assert!(
            p.dropped.is_empty(),
            "熔断没开却排出了 {} 条",
            p.dropped.len()
        );
        assert_eq!(p.to_tokens, p.from_tokens, "没动手就不该有占用变化");
    }
}

// ---------- 熔断开了 ⇒ 提问一条不少、过程一条不留（AC-S2-8 第一句）----------

#[test]
fn every_question_survives_and_old_process_is_gone() {
    let msgs = rounds(6, 4_000);
    let p = plan_of(&msgs, over_line(&msgs), true);
    assert_eq!(p.reason, "planned", "实测 {:?}", p);
    assert_eq!(
        questions(&msgs, &p.dropped),
        vec![
            "第0轮问题",
            "第1轮问题",
            "第2轮问题",
            "第3轮问题",
            "第4轮问题",
            "第5轮问题"
        ],
        "提问一条都不许少，而且顺序不许变"
    );
    assert_eq!(
        kept_calls(&msgs, &p.dropped),
        vec!["c5"],
        "只留最近 {SLIDE_KEEP_RUNS_DEFAULT} 段过程，旧轮的过程该一条不剩"
    );
    assert!(
        p.to_tokens < p.from_tokens,
        "丢了一整段过程却没省字节：{p:?}"
    );
    assert!(
        orphan_results(&msgs, &p.dropped).is_empty(),
        "切完发出去的形状有孤儿回执"
    );
}

/// 摘要行与整理段落行在投影里都是 `Message::User`——它们**必须**被这一层留下：
/// 那是已压过的历史唯一的在场形式，再丢一次就是永久失去。
#[test]
fn a_summarized_line_is_kept_because_it_reads_as_a_question() {
    let mut msgs = rounds(4, 4_000);
    msgs.insert(0, summarized_message("## 目标\n前史摘要"));
    let p = plan_of(&msgs, over_line(&msgs), true);
    assert_eq!(p.reason, "planned");
    assert!(
        questions(&msgs, &p.dropped)
            .iter()
            .any(|c| c.starts_with(SUMMARIZED_MARKER)),
        "摘要行被保护网丢掉了：压过的历史从此无处可寻"
    );
}

// ---------- 不许拆对：assistant 与它的工具回执 ----------

/// 孤儿形状：一条 assistant 声明了 `c0`，而 `c0` 的回隔在**下一条提问之后**
/// （旧日志残段恢复出来的形状）。按段丢弃会把 assistant 判给"旧段"、把回执留在
/// "最近段" ⇒ 发出去的就是一份"回执没有前置声明"的请求，供应商整单拒收。
/// 闭合修正必须把那条 assistant 拉回来；拉回来之后没得可丢，如实报 no-process。
#[test]
fn a_dropped_assistant_takes_its_own_result_with_it() {
    let msgs = vec![
        Message::user("Q0"),
        Message::Assistant {
            content: "我要读文件".into(),
            tool_calls: vec![session_store::ToolCallMsg {
                id: CallId("c0".into()),
                tool: "fs_read".into(),
                args: json!({}),
            }],
        },
        Message::user("Q1"),
        Message::ToolResult {
            call_id: CallId("c0".into()),
            tool: "fs_read".into(),
            args: json!({}),
            output: json!("y".repeat(4_000)),
            error: None,
        },
    ];
    let p = plan_of(&msgs, over_line(&msgs), true);
    assert!(
        orphan_results(&msgs, &p.dropped).is_empty(),
        "闭合修正没生效，实测 dropped={:?} reason={}",
        p.dropped,
        p.reason
    );
    assert_eq!(
        p.reason, "no-process",
        "拉回 assistant 后应当没有可丢的了：{p:?}"
    );
}

/// 常规多轮下也再数一遍同一件事（上一条测的是畸形形状，这条测的就是默认路径）。
#[test]
fn no_split_pair_survives_into_the_request() {
    let msgs = rounds(9, 3_000);
    let p = plan_of(&msgs, over_line(&msgs), true);
    assert_eq!(p.reason, "planned");
    assert!(
        !p.dropped.is_empty() && orphan_results(&msgs, &p.dropped).is_empty(),
        "要么什么都没丢（用例白测），要么丢出了孤儿：{:?}",
        p.dropped
    );
}

// ---------- 另外三条"该不动"的线 ----------

/// 窗口未知 ⇒ 连保护网也不开（裁定 A 在这一层的同一条姿态：没有可信的线就不动手，
/// 哪怕熔断已经开了——这一层省的字节数完全依赖那条线）。
#[test]
fn an_unknown_window_keeps_the_net_off() {
    let msgs = rounds(6, 4_000);
    let p = plan_of(&msgs, None, true);
    assert_eq!(p.reason, "unknown-window");
    assert!(p.dropped.is_empty());
}

#[test]
fn below_the_line_keeps_the_net_off() {
    let msgs = rounds(6, 4_000);
    let p = plan_of(&msgs, under_line(&msgs), true);
    assert_eq!(p.reason, "below-line", "熔断了但量在线下 ⇒ 还轮不到保护网");
    assert!(p.dropped.is_empty());
}

/// 只有一段过程 ⇒ 那一段就是"最近一段"，没有旧的可以丢。
#[test]
fn one_run_left_is_nothing_to_drop() {
    let msgs = rounds(1, 8_000);
    let p = plan_of(&msgs, over_line(&msgs), true);
    assert_eq!(p.reason, "no-process");
    assert!(p.dropped.is_empty());
}

/// 全是提问（没有任何过程）⇒ 同样零动作，而且不许 panic（段表是空的）。
#[test]
fn a_history_of_only_questions_drops_nothing() {
    let msgs: Vec<Message> = (0..6).map(|i| Message::user(format!("Q{i}"))).collect();
    let p = plan_of(&msgs, over_line(&msgs), true);
    assert_eq!(p.reason, "no-process");
    assert!(p.dropped.is_empty());
}
