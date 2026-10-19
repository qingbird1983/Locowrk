//! 上下文区那四枚**数值**设置的面（刀⑥ · AC-S2-10）：缺省 = 本刀之前那颗硬编码
//! 常量、出界一律拒绝（不悄悄钳）、合法值走磁盘往返、旧文件缺键走字段级缺省。
//!
//! 为什么四枚一起测而不是只测刀⑥ 新加的两枚：`compactTargetPct` 与
//! `compactKeepTokens` 是刀④ 加的，当时**只有** `compact/tests.rs` 里把它们当判据
//! 输入用，"越界拒绝"那条写在 `apply_update` 里没人证过。刀⑥ 给它们补界面行时
//! 一并补齐，四枚同族共用一套夹具。
//!
//! ⚠ 本簇不许出现裸数字：缺省与上下界全部从 `settings/context.rs` 的常量取。
//! 测试里手抄一份的话，改了常量会红在测试上而不是红在生产里——那是"同一属性两个
//! 出处"的第三种说谎形态。

use super::*;
use crate::settings::context::{
    COMPACT_KEEP_TOKENS_DEFAULT, COMPACT_KEEP_TOKENS_RANGE, COMPACT_TARGET_PCT_DEFAULT,
    COMPACT_TARGET_PCT_RANGE, MICRO_KEEP_GROUPS_DEFAULT, MICRO_KEEP_GROUPS_RANGE,
    ORGANIZE_THRESHOLD_DEFAULT, SLIDE_KEEP_RUNS_DEFAULT, SLIDE_KEEP_RUNS_RANGE,
};

fn open_defaults(tag: &str) -> SettingsStore {
    SettingsStore::open(&temp_root(tag)).unwrap()
}

/// ①缺省 = 那颗常量（AC-S2-10 的"缺省 = 现状"在这一层的说法）。
/// 这四枚的真身在别处：`MICRO_KEEP_GROUPS_DEFAULT` 对应刀③ 时 `microcompact.rs`
/// 里的 `KEEP_RECENT_GROUPS`，`SLIDE_KEEP_RUNS_DEFAULT` 对应刀⑤ 时的
/// `KEEP_RECENT_RUNS`，另两枚对应刀④ 时 `compact.rs` 里的目标比与保留额度。
/// 抬成设置项后那些常量都必须不存在（判据 SS23a），于是**这里就是它们唯一的家**。
#[test]
fn the_four_numbers_default_to_the_constants() {
    let v = open_defaults("ctx-num-defaults").view();
    assert_eq!(v.context.compact_target_pct, COMPACT_TARGET_PCT_DEFAULT);
    assert_eq!(v.context.compact_keep_tokens, COMPACT_KEEP_TOKENS_DEFAULT);
    assert_eq!(v.context.micro_keep_groups, MICRO_KEEP_GROUPS_DEFAULT);
    assert_eq!(v.context.slide_keep_runs, SLIDE_KEEP_RUNS_DEFAULT);
}

/// ②旧 settings.json 里没有这四键 ⇒ 字段级缺省，零迁移（同刀③/④ 那两批的口径）。
/// 这一条不是冗余：`#[serde(default)]` 挂在结构体上，漏掉它时"整区缺失"仍然走
/// `Default::default()`，但"区在场、只有旧键"就会给字段填 0——那正好把保留组数
/// 变成 0，是比报错更坏的形状。
#[test]
fn old_files_without_these_keys_still_get_the_defaults() {
    let root = temp_root("ctx-num-oldfile");
    std::fs::write(
        root.join(FILE_NAME),
        r#"{"context":{"thresholdTokens":100000,"usageBar":true}}"#,
    )
    .unwrap();
    let v = SettingsStore::open(&root).unwrap().view();
    assert_eq!(v.context.compact_target_pct, COMPACT_TARGET_PCT_DEFAULT);
    assert_eq!(v.context.compact_keep_tokens, COMPACT_KEEP_TOKENS_DEFAULT);
    assert_eq!(v.context.micro_keep_groups, MICRO_KEEP_GROUPS_DEFAULT);
    assert_eq!(v.context.slide_keep_runs, SLIDE_KEEP_RUNS_DEFAULT);
    // 同区那两枚旧键照原样读回：字段级缺省不许顺手把在场的花园铲平
    assert_eq!(v.context.threshold_tokens, 100_000);
    assert!(v.context.usage_bar);
}

/// ③出界一律拒绝，而且**不污染内存态**（悄悄钳到边界 = 骗人：他给什么就该存什么，
/// 存不下就该告诉他存不下）。
#[test]
fn out_of_range_numbers_are_rejected_without_polluting() {
    let store = open_defaults("ctx-num-reject");
    let cases: Vec<(&str, ContextUpdate)> = vec![
        (
            "目标比 0",
            ContextUpdate {
                compact_target_pct: Some(*COMPACT_TARGET_PCT_RANGE.start() - 1),
                ..Default::default()
            },
        ),
        (
            "目标比 上限 +1",
            ContextUpdate {
                compact_target_pct: Some(*COMPACT_TARGET_PCT_RANGE.end() + 1),
                ..Default::default()
            },
        ),
        (
            "保留额度 上限 +1",
            ContextUpdate {
                compact_keep_tokens: Some(*COMPACT_KEEP_TOKENS_RANGE.end() + 1),
                ..Default::default()
            },
        ),
        (
            // 0 组 = 连刚拿到的结果都清 ⇒ 这一层从"减速"变"致盲"，所以区间从 1 起
            "微压缩保留组数 0",
            ContextUpdate {
                micro_keep_groups: Some(*MICRO_KEEP_GROUPS_RANGE.start() - 1),
                ..Default::default()
            },
        ),
        (
            "微压缩保留组数 上限 +1",
            ContextUpdate {
                micro_keep_groups: Some(*MICRO_KEEP_GROUPS_RANGE.end() + 1),
                ..Default::default()
            },
        ),
        (
            "滑窗保留段数 0",
            ContextUpdate {
                slide_keep_runs: Some(*SLIDE_KEEP_RUNS_RANGE.start() - 1),
                ..Default::default()
            },
        ),
    ];
    for (what, update) in cases {
        assert!(
            set_context(&store, update).is_err(),
            "{what} 越界却被接受了（区间常量形同不存在）"
        );
    }
    let v = store.view();
    assert_eq!(v.context.compact_target_pct, COMPACT_TARGET_PCT_DEFAULT);
    assert_eq!(v.context.compact_keep_tokens, COMPACT_KEEP_TOKENS_DEFAULT);
    assert_eq!(v.context.micro_keep_groups, MICRO_KEEP_GROUPS_DEFAULT);
    assert_eq!(v.context.slide_keep_runs, SLIDE_KEEP_RUNS_DEFAULT);
}

/// ④合法值（各取区间两端，含边界本身）存进去 ⇒ 重开磁盘读回同一份。
/// 取边界而不是中间值：边界是"含不含"最容易写错的那一格（`contains` 对
/// `RangeInclusive` 含两端，写成 `start < v && v < end` 就悄悄少两个合法档）。
#[test]
fn legal_numbers_roundtrip_through_disk() {
    let root = temp_root("ctx-num-roundtrip");
    let store = SettingsStore::open(&root).unwrap();
    set_context(
        &store,
        ContextUpdate {
            compact_target_pct: Some(*COMPACT_TARGET_PCT_RANGE.end()),
            compact_keep_tokens: Some(*COMPACT_KEEP_TOKENS_RANGE.start()),
            micro_keep_groups: Some(*MICRO_KEEP_GROUPS_RANGE.end()),
            slide_keep_runs: Some(*SLIDE_KEEP_RUNS_RANGE.start()),
            ..Default::default()
        },
    )
    .unwrap();
    let v = SettingsStore::open(&root).unwrap().view();
    assert_eq!(
        v.context.compact_target_pct,
        *COMPACT_TARGET_PCT_RANGE.end()
    );
    // 0 = "不留近期原文"是合法档（极端但用户可能想要），不是"没设"
    assert_eq!(v.context.compact_keep_tokens, 0);
    assert_eq!(v.context.micro_keep_groups, *MICRO_KEEP_GROUPS_RANGE.end());
    assert_eq!(v.context.slide_keep_runs, *SLIDE_KEEP_RUNS_RANGE.start());
    // 同一趟没给的键保持不变（`ContextUpdate` 的 default = None = 不动）
    assert_eq!(v.context.threshold_tokens, ORGANIZE_THRESHOLD_DEFAULT);
}
