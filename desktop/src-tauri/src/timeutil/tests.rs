//! 时钟收敛等值测试（批 F·F1）：新入口必须与**被删的九份本地实现**逐字节同值。
//!
//! 参考实现是收敛前源码的原文照抄（出处标在注释里），不是手写期望值——这样
//! "取整路径变了""回落值变了"这类悄悄漂的行为会当场红。

use super::{epoch_ms_of, now_ms, now_secs, secs_of_millis};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// runner.rs:231 原文（唯一与其余不同的一条：用 `unwrap_or_default()` 而非 `unwrap_or(0)`）
fn old_runner_shape(now: SystemTime) -> i64 {
    now.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

/// organize/mod.rs:112、organize/tree_tools.rs:280、llm/diagnose.rs:206、
/// scheduler/store.rs:65、commands/sessions.rs:293 原文（六处同形）
fn old_ms_map_shape(now: SystemTime) -> i64 {
    now.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// llm/catalog.rs:318 原文（u64 形状）
fn old_catalog_shape(now: SystemTime) -> u64 {
    now.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// media/mod.rs:289、notifications.rs:106、browser/console.rs:64 原文（u128 形状）
fn old_u128_shape(now: SystemTime) -> u128 {
    now.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

/// autodisable.rs:20、commands/plugins.rs:99、usage.rs:275 原文（秒，直接取
/// `as_secs()`；`memory.rs` / `memory_reconcile.rs` 两处已随 S4 刀③c 删除）
fn old_secs_shape(now: SystemTime) -> i64 {
    now.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn samples() -> Vec<SystemTime> {
    vec![
        UNIX_EPOCH,
        UNIX_EPOCH + Duration::from_millis(1),
        UNIX_EPOCH + Duration::from_millis(999),
        UNIX_EPOCH + Duration::from_millis(1000),
        UNIX_EPOCH + Duration::from_millis(1001),
        UNIX_EPOCH + Duration::from_secs(1_791_543_571),
        UNIX_EPOCH + Duration::new(1_700_000_000, 999_999_999),
        // 早于 epoch：旧实现走 `unwrap_or(0)` / `unwrap_or_default()` 分支
        UNIX_EPOCH - Duration::from_secs(1),
        UNIX_EPOCH - Duration::from_millis(1),
    ]
}

#[test]
fn every_shape_matches_the_removed_local_implementations() {
    for t in samples() {
        let ms = epoch_ms_of(t);
        assert_eq!(ms as i64, old_ms_map_shape(t), "i64 毫秒形状");
        assert_eq!(
            ms as i64,
            old_runner_shape(t),
            "runner 的 unwrap_or_default 形状"
        );
        assert_eq!(ms as u64, old_catalog_shape(t), "catalog 的 u64 形状");
        assert_eq!(ms, old_u128_shape(t), "id 后缀用的 u128 形状");
        assert_eq!(
            secs_of_millis(ms),
            old_secs_shape(t),
            "秒：换算法必须等于直接 as_secs()"
        );
    }
}

#[test]
fn clock_before_epoch_falls_back_to_zero_like_the_old_code() {
    let before = UNIX_EPOCH - Duration::from_secs(3_600);
    assert_eq!(
        epoch_ms_of(before),
        0,
        "旧实现全部回落 0，新入口不能 panic 也不能给负数"
    );
    assert_eq!(secs_of_millis(epoch_ms_of(before)), 0);
}

#[test]
fn the_two_units_read_the_same_clock() {
    // 两把尺必须同源（一次读毫秒一次读秒，中间跨秒是允许的，但不能漂过 1 秒）。
    for _ in 0..200 {
        let ms = now_ms();
        let secs = now_secs();
        assert!(
            (ms / 1000 - secs).abs() <= 1,
            "now_ms/1000 与 now_secs 必须读同一个钟：ms={ms} secs={secs}"
        );
        assert!(
            secs > 1_000_000_000,
            "读到的秒数量级不对（时钟异常？）：{secs}"
        );
    }
}
