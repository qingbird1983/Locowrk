//! 内核时钟尺的行为用例（Q12·第二刀 v2.205）。
//!
//! 两条各钉一件事：**读数真是"现在"**（被前后两个独立读数夹住 ⇒ 不是常量、不是 0），
//! 以及**两个单位彼此对得上**（秒 = 毫秒所在的那一秒，允许跨秒边界的 ±1）。
//! 时钟不可用 ⇒ 0 那条兜底支不造注入点：为它加一层"可替换时钟"机制换来的只是
//! 一条能绿的用例，可观察行为没有变化（同 `desktop/src-tauri/src/timeutil.rs` 的裁定）。

use super::{now_ms, now_secs};
use std::time::{SystemTime, UNIX_EPOCH};

fn epoch_ms_of(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[test]
fn now_ms_is_bounded_by_two_independent_reads() {
    let before = epoch_ms_of(SystemTime::now());
    let read = now_ms();
    let after = epoch_ms_of(SystemTime::now());
    assert!(
        (before..=after + 1).contains(&read),
        "读数 {read} 不在 [{before}, {after}] 内 ⇒ 尺不是'读现在'"
    );
    // 秒级墙钟量级自检：时钟若被拨到 1970 附近，这条先红，避免"恒 0"冒充有效读数。
    assert!(read > 1_700_000_000_000, "毫秒读数异常小：{read}");
}

#[test]
fn both_units_agree_within_one_second() {
    let ms = now_ms();
    let secs = now_secs();
    let ms_in_secs = ms / 1000;
    let drift = (ms_in_secs - secs).abs();
    assert!(
        drift <= 1,
        "两单位对不上：now_ms/1000 = {ms_in_secs} 而 now_secs = {secs}"
    );
    // 两条读数之间不许出现"取整路径分家"级别的时差（历史上内联版用 as_secs、
    // 桌面版用 ms/1000，等值成立但差一秒的窗口确实存在 ⇒ 只放宽到 1 秒）。
    assert!(
        ms > 0 && secs > 0,
        "读数不得为 0（时钟不可用的兜底值不该在正常环境出现）"
    );
}
