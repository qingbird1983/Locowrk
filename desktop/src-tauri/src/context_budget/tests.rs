//! 刀① 判据：锚点搬家同值 + 有效阈值只扣一次（specs/09 AC-S2-1 / AC-S2-2）。
//! 刀④a（S3 · ⓟ）加判据：占用分子三档取最大，且**进度条与压缩同吃这一个函数**。

use memory_tree::AnchorSource;
use session_store::{Session, SessionEvent};

use super::{anchor, effective_threshold, occupancy, output_reserve};

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
/// 数值 9,500 取自历史夹具的同一档读数；搬家前后断言一字未改，就是"读数没动"
/// 的证据。
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
    assert_eq!(
        output_reserve(None, 21_000),
        21_000,
        "缺省 32K 与上限 21K 取小"
    );
    assert_eq!(
        output_reserve(Some(64_000), 21_000),
        21_000,
        "大上限被 21K 削平"
    );
    assert_eq!(output_reserve(Some(8_192), 21_000), 8_192, "小上限原样扣");
}

/// AC-S2-2 主判据：`min(滑杆, 窗口 − 预留)`，输出侧**只扣一次**。
/// 夹具照 spec 那两组：窗口 200K / 输出上限 64K / 滑杆 150K ⇒ 150K（滑杆说话）；
/// 滑杆 400K ⇒ 179K（窗口封顶说话）。两个负对照钉"扣两遍"（158K）与
/// "扣整颗输出上限"（136K）——那正是本判据要挡住的两种历史形状。
#[test]
fn effective_threshold_is_capped_by_the_window_once() {
    assert_eq!(
        effective_threshold(150_000, Some(200_000), Some(64_000), 21_000),
        150_000,
        "滑杆低于封顶 ⇒ 用户意图说话"
    );
    let capped = effective_threshold(400_000, Some(200_000), Some(64_000), 21_000);
    assert_eq!(capped, 179_000, "滑杆高于封顶 ⇒ 窗口减一次预留说话");
    assert_ne!(capped, 158_000, "158K = 预留扣了两遍的形状");
    assert_ne!(capped, 136_000, "136K = 把整颗输出上限扣掉的 legacy 形状");
}

/// S4（specs/11 刀①）之后上限恒为 [`super::OUTPUT_RESERVE_CAP_TOKENS`]，但算式仍
/// 保留第四枚 `cap` 参数：这一条钉的是"预留取 min，而不是永远吃常量"这一半的算术。
/// 生产调用方传固定常量，测试传别的值只为证明算式没被写死。
#[test]
fn effective_threshold_follows_the_cap_argument() {
    // 窗口 200K / 滑杆 400K / 模型输出上限 64K。
    assert_eq!(
        effective_threshold(400_000, Some(200_000), Some(64_000), 21_000),
        179_000,
        "缺省上限 21K ⇒ 空位 179K"
    );
    assert_eq!(
        effective_threshold(400_000, Some(200_000), Some(64_000), 8_000),
        192_000,
        "上限调到 8K ⇒ 空位 192K（阈值跟着变）"
    );
    assert_eq!(
        effective_threshold(400_000, Some(200_000), Some(64_000), 0),
        200_000,
        "上限调到 0 ⇒ 不为输出让位，阈值 = 窗口"
    );
}

/// 2026-10-19 裁定 A 的**这一半**：窗口取不到 ⇒ 不假定窗口，阈值原样吃滑杆
/// （进度条于是仍有分母可读）。"因此不自动压缩"那一半归刀④ —— 现在没有自动
/// 压缩的执行者，预造判据就是取证①点名的"存在但从未接线"。
#[test]
fn unknown_window_keeps_the_slider() {
    assert_eq!(
        effective_threshold(100_000, None, None, 21_000),
        100_000,
        "窗口未知不许拿假定的 200K 去封顶"
    );
}

/// 病态配置（窗口比预留还小）如实落到 0，不偷偷兜一个下限：ZCode
/// `policy.ts:72` 用的就是 `max(0, …)`。0 的含义由熔断与 `compaction_armed`
/// 兜住空转，写死数会造出"第二把尺"（specs/09 AC-S2-2 的"只有一个出处"）。
#[test]
fn a_window_smaller_than_the_reserve_floors_at_zero() {
    assert_eq!(effective_threshold(100_000, Some(10_000), None, 21_000), 0);
}

// ---- 刀④a（S3 · ⓟ）：占用分子三档 ----

/// 第 2 档的夹具：在上报**之后**补一条压缩事件 ⇒ 最后一次 `LlmUsage` 过期。
/// `kept_messages = 2` = 只留最近两条 + 一条摘要标记，投影后的历史显著短于上报那颗数。
fn compacted_after_report() -> Session {
    let mut s = seeded(Some(9_500));
    s.append(SessionEvent::ContextCompacted {
        kept_messages: 2,
        summary: "前面几轮已折叠".into(),
        from_tokens: 9_500,
        to_tokens: 400,
        strategy: "compact".into(),
        ts: 1,
    });
    s
}

/// 夹具自证：压后投影确实**小于**那颗过期上报。这条红了说明夹具没造出场景，
/// 后面那两条的"取小者"断言就是空转（假绿）。
#[test]
fn compaction_really_shrinks_the_projection() {
    let s = compacted_after_report();
    let est = kernel_agent::estimate_tokens(&s.derive_messages());
    assert!(
        est < 9_500,
        "夹具没造出「过期上报比压后投影大」：实测 {est}"
    );
}

/// 第 1 档 = 锚点同值。这一条就是 ⓟ 的本体：进度条与压缩判定**不许**各有分子。
/// 断言取 `anchor(&s).0` 而不是手抄 9,500——抄数字的话，把 `occupancy` 写成
/// 另一把尺也照样绿（它只要恰好给出同一个数）。
#[test]
fn occupancy_with_a_fresh_report_is_the_anchor_itself() {
    let s = seeded(Some(9_500));
    assert_eq!(
        occupancy(&s, 0),
        anchor(&s).0,
        "有新鲜上报时分子必须逐字等于锚点，不是另算一遍"
    );
}

/// 第 2 档：上报过期 ⇒ 分子改吃全量估算。旧形状里这一档只有压缩面有，进度条没有
/// （它那时看的是"目录 + 尾巴"）⇒ 同一轮两处不同数。
#[test]
fn occupancy_drops_a_report_that_compaction_made_stale() {
    let s = compacted_after_report();
    let est = kernel_agent::estimate_tokens(&s.derive_messages());
    assert_eq!(
        occupancy(&s, 0),
        est,
        "最后一次上报落在压缩事件之前 ⇒ 那颗数描述的是压缩前的窗口"
    );
    assert_ne!(
        occupancy(&s, 0),
        9_500,
        "照过期上报判定 = 每个 step 再压一次的形状"
    );
}

/// 第 3 档：被超窗拒收那颗数顶上来（它既没有上报也没有压缩事件，前两档追不到）。
#[test]
fn occupancy_takes_the_refusal_when_it_is_the_largest() {
    let s = seeded(Some(9_500));
    assert_eq!(
        occupancy(&s, 240_000),
        240_000,
        "被拒现场只能由第 3 档表达，否则触发线永远停在被拒之前（台账 ⓜ）"
    );
}

/// 三档取**最大**而不是"换尺"的两条边界：缺席当 0（不动读数）、比读数小也不动。
/// 少了这一条，把 `.max()` 写成"有拒收就用拒收"（无条件覆盖）也照样绿。
#[test]
fn a_smaller_or_absent_refusal_does_not_move_the_reading() {
    let s = seeded(Some(9_500));
    assert_eq!(
        occupancy(&s, 0),
        9_500,
        "缺席 = 0 = 不参与，不许当无穷也不许当跳过"
    );
    assert_eq!(occupancy(&s, 100), 9_500, "更小的旧拒收不许把读数压回去");
}
