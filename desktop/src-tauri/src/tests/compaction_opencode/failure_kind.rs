//! specs/13 刀⑦b（**AC-29**）：压缩层**按档位**认失败，**不猜文案**。
//!
//! 判据的承重点：改前压缩层从**错误文本**里认「撞输出上限 / 被内容过滤 / 字节
//! 太大」三枚（`compact/deep/send.rs` 的三个文本匹配函数）⇒ 供应商换个措辞就归
//! 错档，界面那句「为什么没压成」直接说错。改后**档位**随错误上来，压缩层按档
//! 认 ⇒ **把文案换成任意串也不影响归类**。
//!
//! 六条用例：五档各一条（正向）+ 一条**反向**（文案写着别的东西也不许改判）。
//! 反向那一半最承重：改前那种实现会把"文案里写着 finish_reason=length"的
//! 普通失败误判成"撞输出上限"。

use super::*;

use crate::compact::outcome::{BUDGET, CONTENT_FILTER, OUTPUT_LIMIT, PROVIDER_ERROR};
use crate::llm::failure::FailureKind;

/// 造一趟压缩：喂一个**带档位**的失败，拿回（结果、发出的趟数）。
///
/// 用量 150K > 门槛 100K（窗口 200K × 50%）⇒ 自动那一路会起步。
async fn run_with_failure(
    tag: &str,
    kind: FailureKind,
    message: &str,
) -> (crate::compact::Outcome, usize) {
    let script = Script::new(vec![Reply::FailKind(kind, message.to_string())]);
    let (cp, _root) = compactor(tag, Arc::clone(&script), true, 50);
    let mut s = session_with_usage(150_000, 4, 200);
    let out = cp.run(&mut s, tag, &policy(200_000, 50), &hooks()).await;
    (out, script.calls())
}

/// 撞输出上限（`OutputLimit`）⇒ 判据名 `output-limit`，且**不再发第二趟**
/// （输出上限不是"太长"⇒ 收缩对它没用）。
#[tokio::test]
async fn the_output_limit_kind_becomes_the_output_limit_reason() {
    let (out, calls) = run_with_failure("ac29-output", FailureKind::OutputLimit, "任意串").await;
    assert_eq!(
        out.reason(),
        Some(OUTPUT_LIMIT),
        "撞输出上限 ⇒ `output-limit` 那一档：{out:?}"
    );
    assert_eq!(calls, 1, "输出上限不许再发一趟（收缩对它没用）");
}

/// 被内容过滤（`ContentFilter`）⇒ 判据名 `content-filter`。
#[tokio::test]
async fn the_content_filter_kind_becomes_the_content_filter_reason() {
    let (out, calls) = run_with_failure("ac29-filter", FailureKind::ContentFilter, "任意串").await;
    assert_eq!(
        out.reason(),
        Some(CONTENT_FILTER),
        "被内容过滤 ⇒ `content-filter` 那一档：{out:?}"
    );
    assert_eq!(calls, 1, "内容过滤不许重试（重发还是被拦）");
}

/// 预算耗尽（`Budget`）⇒ 判据名 `budget`。
#[tokio::test]
async fn the_budget_kind_becomes_the_budget_reason() {
    let (out, calls) = run_with_failure("ac29-budget", FailureKind::Budget, "任意串").await;
    assert_eq!(
        out.reason(),
        Some(BUDGET),
        "预算耗尽 ⇒ `budget` 那一档：{out:?}"
    );
    assert_eq!(calls, 1, "预算耗尽不许再发一趟");
}

/// 其它（`Other`）⇒ 判据名 `provider-error`。
#[tokio::test]
async fn the_other_kind_becomes_the_provider_error_reason() {
    let (out, calls) = run_with_failure("ac29-other", FailureKind::Other, "任意串").await;
    assert_eq!(
        out.reason(),
        Some(PROVIDER_ERROR),
        "其它失败 ⇒ `provider-error` 那一档：{out:?}"
    );
    assert_eq!(calls, 1, "普通失败不许自己重发");
}

/// 上下文超窗（`ContextOverflow`）⇒ **收缩后再发**（这一档能自愈）。
#[tokio::test]
async fn the_context_overflow_kind_shrinks_and_retries() {
    let script = Script::new(vec![Reply::FailKind(
        FailureKind::ContextOverflow,
        "任意串".into(),
    )]);
    let (cp, _root) = compactor("ac29-overflow", Arc::clone(&script), true, 50);
    let mut s = session_with_usage(150_000, 4, 200);
    let out = cp
        .run(&mut s, "ac29-overflow", &policy(200_000, 50), &hooks())
        .await;
    assert!(
        script.calls() > 1,
        "超窗 ⇒ 收缩后再发（这一档能自愈）：实发 {} 趟",
        script.calls()
    );
    let reqs = script.requests();
    let first = crate::compact::deliver::estimate_request(&reqs[0]);
    let second = crate::compact::deliver::estimate_request(&reqs[1]);
    assert!(second < first, "第二趟更小：{first} → {second}");
    assert_eq!(
        out.reason(),
        Some(PROVIDER_ERROR),
        "收缩带用尽 ⇒ 如实报供应商错误：{out:?}"
    );
}

/// 字节太大（`PayloadTooLarge`）⇒ **改纯文本再发一次**（上游同款：多半是内联媒体）。
#[tokio::test]
async fn the_payload_too_large_kind_flattens_and_retries() {
    let (out, calls) =
        run_with_failure("ac29-payload", FailureKind::PayloadTooLarge, "任意串").await;
    assert_eq!(
        calls, 2,
        "字节太大 ⇒ 改纯文本再发一次（上游 `compaction.ts:448-453`）"
    );
    assert_eq!(
        out.reason(),
        Some(PROVIDER_ERROR),
        "压成文本还太大 ⇒ 如实报供应商错误：{out:?}"
    );
}

/// **反向**：文案写着别的东西**不许**改判——档位说是别的，就是别的。
///
/// 这一条是本刀的**承重判据**：改前那种实现（从文案里找 "finish_reason=length"）
/// 会把这一条误判成"撞输出上限" ⇒ 界面说错了原因，而压缩面照跑不误、没有任何
/// 一条别的用例会红。
#[tokio::test]
async fn the_message_never_overrides_the_kind() {
    let (out, calls) = run_with_failure(
        "ac29-anti",
        FailureKind::Other,
        "模型输出在最大输出上限处被截断（finish_reason=length）",
    )
    .await;
    assert_eq!(
        out.reason(),
        Some(PROVIDER_ERROR),
        "档位说别的就是别的（文案不许改判）：{out:?}"
    );
    assert_eq!(calls, 1, "也不许因为文案像而多发一趟");
}
