//! classify / filter_key 全表测试（exposure 文档 §8.6 + §8.7 逐行对照）。

use super::{classify, filter_key, IdleLevel, IdleUnit};

const NOW: i64 = 1_800_000_000;
const DAY: i64 = 86_400;
/// 安装时间固定在 400 天前（从未使用用例的安装时长基准）。
const INSTALLED: i64 = NOW - 400 * DAY;

fn used_days_ago(days: i64) -> Option<i64> {
    Some(NOW - days * DAY)
}

#[test]
fn used_table_matches_doc_8_6() {
    let rows: &[(i64, IdleLevel, Option<u32>, IdleUnit)] = &[
        (0, IdleLevel::Fresh, None, IdleUnit::Today),
        (1, IdleLevel::Fresh, None, IdleUnit::Today),
        (2, IdleLevel::Fresh, None, IdleUnit::ThisWeek),
        (6, IdleLevel::Fresh, None, IdleUnit::ThisWeek),
        (7, IdleLevel::Fresh, None, IdleUnit::LastWeek),
        (13, IdleLevel::Fresh, None, IdleUnit::LastWeek),
        (14, IdleLevel::Normal, None, IdleUnit::LastMonth),
        (29, IdleLevel::Normal, None, IdleUnit::LastMonth),
        (30, IdleLevel::Idle, Some(1), IdleUnit::Month),
        (59, IdleLevel::Idle, Some(1), IdleUnit::Month),
        (60, IdleLevel::Idle, Some(2), IdleUnit::Month),
        (89, IdleLevel::Idle, Some(2), IdleUnit::Month),
        (90, IdleLevel::Stale, Some(3), IdleUnit::Month),
        (179, IdleLevel::Stale, Some(3), IdleUnit::Month),
        (180, IdleLevel::Abandoned, None, IdleUnit::HalfYear),
        (364, IdleLevel::Abandoned, None, IdleUnit::HalfYear),
    ];
    for (days, level, value, unit) in rows {
        let info = classify(NOW, INSTALLED, used_days_ago(*days));
        assert_eq!(info.level, *level, "{days} 天前用过：level");
        assert_eq!(info.value, *value, "{days} 天前用过：value");
        assert_eq!(info.unit, Some(*unit), "{days} 天前用过：unit");
    }
    // ≥365：整年倍数，最少 1
    let y1 = classify(NOW, INSTALLED, used_days_ago(365));
    assert_eq!((y1.level, y1.value), (IdleLevel::Abandoned, Some(1)));
    let y2 = classify(NOW, INSTALLED, used_days_ago(412));
    assert_eq!(y2.value, Some(1), "412 天 = 1 年多（§8.4 文案例）");
    let y3 = classify(NOW, INSTALLED, used_days_ago(800));
    assert_eq!(y3.value, Some(2));
    assert_eq!(y3.unit, Some(IdleUnit::Year));
}

#[test]
fn never_used_table_matches_doc_8_7() {
    let rows: &[(i64, IdleLevel, Option<u32>, IdleUnit)] = &[
        (0, IdleLevel::Fresh, None, IdleUnit::JustInstalled),
        (13, IdleLevel::Fresh, None, IdleUnit::JustInstalled),
        (14, IdleLevel::NeverUsed, None, IdleUnit::NeverUsed),
        (30, IdleLevel::NeverUsed, None, IdleUnit::NeverUsed),
        (31, IdleLevel::NeverUsed, Some(1), IdleUnit::NeverUsed),
        (90, IdleLevel::NeverUsed, Some(1), IdleUnit::NeverUsed),
        (91, IdleLevel::NeverUsed, Some(3), IdleUnit::NeverUsed),
        (180, IdleLevel::NeverUsed, Some(3), IdleUnit::NeverUsed),
        (181, IdleLevel::NeverUsed, Some(6), IdleUnit::NeverUsed),
    ];
    for (age, level, value, unit) in rows {
        let info = classify(NOW, NOW - age * DAY, None);
        assert_eq!(info.level, *level, "装了 {age} 天从未用：level");
        assert_eq!(info.value, *value, "装了 {age} 天从未用：value");
        assert_eq!(info.unit, Some(*unit), "装了 {age} 天从未用：unit");
    }
}

#[test]
fn filter_keys_nest_like_doc_9_2_chips() {
    let key_of = |days_ago: Option<i64>| {
        let info = classify(NOW, INSTALLED, days_ago.map(|d| NOW - d * DAY));
        filter_key(&info)
    };
    assert_eq!(key_of(Some(3)), "recent");
    assert_eq!(key_of(Some(29)), "recent");
    assert_eq!(key_of(Some(45)), "gt1m");
    assert_eq!(key_of(Some(89)), "gt1m");
    assert_eq!(key_of(Some(120)), "gt3m");
    assert_eq!(key_of(Some(200)), "gt6m");
    // 从未使用：按安装时长进档（基准夹具 = 400 天前装的 → 半年以上档）
    assert_eq!(key_of(None), "gt6m");
    let fresh_install = classify(NOW, NOW - 100 * DAY, None);
    assert_eq!(
        filter_key(&fresh_install),
        "gt3m",
        "§8.7：91–180 天 = 3 个多月档"
    );
    let half_year_unused = classify(NOW, NOW - 200 * DAY, None);
    assert_eq!(filter_key(&half_year_unused), "gt6m");
}

#[test]
fn idle_info_serializes_snake_case() {
    let info = classify(NOW, INSTALLED, used_days_ago(45));
    let v = serde_json::to_value(&info).unwrap();
    assert_eq!(v["level"], "idle");
    assert_eq!(v["unit"], "month");
    assert_eq!(v["value"], 1);
}
