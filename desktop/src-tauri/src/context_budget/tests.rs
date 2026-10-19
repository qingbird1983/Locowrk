//! 刀① 判据：锚点搬家同值 + 有效阈值只扣一次（specs/09 AC-S2-1 / AC-S2-2）。

use memory_tree::AnchorSource;
use session_store::{Session, SessionEvent};

use super::{anchor, effective_threshold, output_reserve};

/// 六轮历史；`usage` = Some 时追加一条供应商上报（input 9,500）。
fn seeded(usage: Option<u64>) -> Session {
    let mut s = Session::new();
    for i in 0..6 {
        s.append(SessionEvent::UserMessage {
            content: format!("历史第{i}轮问题"),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: format!("历史第{i}轮回答，含具体数字 {i}00。"),
            tool_calls: vec![],
        });
    }
    if let Some(input) = usage {
        s.append(SessionEvent::LlmUsage {
            input,
            output: 10,
            cache_read: 0,
            cache_write: 0,
            reasoning: 0,
            total: input + 10,
            duration_ms: 1,
        });
    }
    s
}

/// AC-S2-1 第一档：有供应商上报 ⇒ 锚点就是那个真实 input，来源标 `LlmUsage`。
/// 数值 9,500 与 `organize/tests/pressure.rs` 那批既有夹具同数——搬家后那些用例
/// 一字未改仍绿，就是"读数没动"的证据。
#[test]
fn anchor_prefers_the_reported_input() {
    let (tokens, source) = anchor(&seeded(Some(9_500)));
    assert_eq!(tokens, 9_500);
    assert_eq!(source, AnchorSource::LlmUsage, "锚点来源必须如实标注");
}

/// AC-S2-1 第二档：无上报 ⇒ 回落**全量历史字符估算**（不是树组装口径；两种回落
/// 的分工写在模块头注里）。断言取的是 `estimate_tokens` 本身而不是某个手抄数字——
/// 抄数字的话，改估算尺时这条用例只会跟着漂，证不出"回落走的是哪把尺"。
#[test]
fn anchor_falls_back_to_the_full_estimate() {
    let s = seeded(None);
    let (tokens, source) = anchor(&s);
    assert_eq!(source, AnchorSource::Estimate);
    assert_eq!(
        tokens,
        kernel_agent::estimate_tokens(&s.derive_messages()),
        "无上报时的锚点 = 全量字符估算这一把尺"
    );
    assert!(tokens > 0, "六轮历史不可能估成 0（0 = 估算根本没跑）");
}

/// AC-S2-2 的预留量三档。第三档（8,192 < 21K）是 `min` 的另一半：只测"大值被
/// 削平"的话，把 `output_reserve` 写成常量 21_000 也照样绿。
#[test]
fn reserve_takes_the_smaller_of_ceiling_and_cap() {
    assert_eq!(output_reserve(None), 21_000, "缺省 32K 与上限 21K 取小");
    assert_eq!(output_reserve(Some(64_000)), 21_000, "大上限被 21K 削平");
    assert_eq!(output_reserve(Some(8_192)), 8_192, "小上限原样扣");
}

/// AC-S2-2 主判据：`min(滑杆, 窗口 − 预留)`，输出侧**只扣一次**。
/// 夹具照 spec 那两组：窗口 200K / 输出上限 64K / 滑杆 150K ⇒ 150K（滑杆说话）；
/// 滑杆 400K ⇒ 179K（窗口封顶说话）。两个负对照钉"扣两遍"（158K）与
/// "扣整颗输出上限"（136K）——那正是本判据要挡住的两种历史形状。
#[test]
fn effective_threshold_is_capped_by_the_window_once() {
    assert_eq!(
        effective_threshold(150_000, Some(200_000), Some(64_000)),
        150_000,
        "滑杆低于封顶 ⇒ 用户意图说话"
    );
    let capped = effective_threshold(400_000, Some(200_000), Some(64_000));
    assert_eq!(capped, 179_000, "滑杆高于封顶 ⇒ 窗口减一次预留说话");
    assert_ne!(capped, 158_000, "158K = 预留扣了两遍的形状");
    assert_ne!(capped, 136_000, "136K = 把整颗输出上限扣掉的 legacy 形状");
}

/// 2026-10-19 裁定 A 的**这一半**：窗口取不到 ⇒ 不假定窗口，阈值原样吃滑杆
/// （进度条于是仍有分母可读）。"因此不自动压缩"那一半归刀④ —— 现在没有自动
/// 压缩的执行者，预造判据就是取证①点名的"存在但从未接线"。
#[test]
fn unknown_window_keeps_the_slider() {
    assert_eq!(
        effective_threshold(100_000, None, None),
        100_000,
        "窗口未知不许拿假定的 200K 去封顶"
    );
}

/// 病态配置（窗口比预留还小）如实落到 0，不偷偷兜一个下限：ZCode
/// `policy.ts:72` 用的就是 `max(0, …)`。0 的含义由熔断与 `compaction_armed`
/// 兜住空转，写死数会造出"第二把尺"（specs/09 AC-S2-2 的"只有一个出处"）。
#[test]
fn a_window_smaller_than_the_reserve_floors_at_zero() {
    assert_eq!(effective_threshold(100_000, Some(10_000), None), 0);
}
