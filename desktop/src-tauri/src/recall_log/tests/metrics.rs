//! 派生指标判据（`RecallLog::stats()` 的两个比率：空块率 / 注入产出率）。
//!
//! 分子分母全部按**键位集合**算而不是按次数——同一 key 查五遍不抬高读数。
//! 这是《Memory Learning Mechanism》§6.1「学习信号可能被『检索即命中』污染」的第一道防线。

use crate::recall_log::RecallLog;

use super::temp_dir;

#[test]
fn stats_report_none_instead_of_zero_when_nothing_was_recorded() {
    // 空表上 0.0 会被读成「注入了但都不空」——两个不同结论，必须用 None 分开。
    let log = RecallLog::at(temp_dir("nostats").join("usage.sqlite3"));
    let st = log.stats().unwrap();
    assert_eq!((st.injections, st.empty_injections), (0, 0));
    assert_eq!(st.store_empty_ratio, None);
    // Q1b 的两个读数同口径：没有注入过任何 key 时 `injection_yield` 也是 None，
    // 不是 0.0（0.0 会被读成"注入了但模型一条没用上"）。
    assert_eq!((st.injected_keys, st.yielded_keys), (0, 0));
    assert_eq!(st.injection_yield, None);
}

// ------------------------------------------------- Q1b：injection_yield 的口径

/// 注入档与检索档在同一张表里对账：分母 = 注入过的 **key 去重全集**，分子 = 其中被
/// 检索回来的**交集**。**重复检索不抬高读数**（同 key 查五遍还是 1）——稿 §6.1 说的
/// 「学习信号可能被『检索即命中』污染」，第一道防线就是把计数换成集合。
#[test]
fn yield_counts_distinct_injected_keys_that_came_back() {
    let log = RecallLog::at(temp_dir("yield").join("usage.sqlite3"));
    log.note_inject(
        "s1",
        "",
        9,
        3,
        &["a".to_string(), "b".to_string(), "c".to_string()],
        std::time::Duration::from_micros(3),
        None,
    )
    .unwrap();
    // 注入档再出一行（另一个键位），keys 与上面重叠 ⇒ 分母仍是 3，不是 4+。
    log.note_inject(
        "s2",
        "F:/ws/a",
        9,
        2,
        &["a".to_string(), "b".to_string()],
        std::time::Duration::from_micros(3),
        None,
    )
    .unwrap();

    for _ in 0..5 {
        log.note_receipt(
            "memory_read",
            "s1",
            "",
            1,
            &["a".to_string()],
            std::time::Duration::from_micros(1),
        )
        .unwrap();
    }
    // `x` 没被注入过：它进分子的话就是"检索到的都算注入产出"，比率能冲到 1 以上。
    log.note_receipt(
        "memory_search",
        "s1",
        "",
        2,
        &["a".to_string(), "x".to_string()],
        std::time::Duration::from_micros(1),
    )
    .unwrap();

    let st = log.stats().unwrap();
    assert_eq!(st.injected_keys, 3, "分母 = 注入过的 key 去重全集");
    assert_eq!(st.yielded_keys, 1, "只有注入过的 key 算产出（a）");
    assert_eq!(st.injection_yield, Some(1.0 / 3.0));
}

/// **不带 key 的回执对分子零贡献**：`memory_project_history` 给的是标题不是 memory key，
/// 它的行必须落账（"模型走没走时间线"是有用的观测）但不能进命中率——否则 §2.3(c) 那个
/// 分子写法就会把一个恒 0 的比值冒充成指标。
#[test]
fn keyless_receipts_do_not_move_the_yield_numerator() {
    let log = RecallLog::at(temp_dir("keyless").join("usage.sqlite3"));
    log.note_inject(
        "s1",
        "",
        1,
        1,
        &["a".to_string()],
        std::time::Duration::from_micros(1),
        None,
    )
    .unwrap();
    log.note_receipt(
        "memory_project_history",
        "s1",
        "F:/ws/a",
        4,
        &[],
        std::time::Duration::from_micros(1),
    )
    .unwrap();
    let st = log.stats().unwrap();
    assert_eq!((st.injected_keys, st.yielded_keys), (1, 0));
    assert_eq!(
        st.injection_yield,
        Some(0.0),
        "有注入、无回查 = 0.0（与 None 是两个结论）"
    );
    assert_eq!(
        log.recent(10).unwrap()[0].returned,
        4,
        "但它的交付条数仍然在册"
    );
}

/// 检索档不许渗进注入档的比率：`store_empty_ratio` 的分母是**注入行数**，回执行再多
/// 也不改变它（`returned = 0` 的回执尤其不能算"空块"——那是"查了但没有"，不是"没东西可注入"）。
/// 这条判据盯的是 mode 取值：把回执写成 `'inject'` 当场红。
#[test]
fn receipts_do_not_enter_the_injection_side_ratios() {
    let log = RecallLog::at(temp_dir("crosstalk").join("usage.sqlite3"));
    log.note_inject(
        "s1",
        "",
        2,
        2,
        &["a".to_string(), "b".to_string()],
        std::time::Duration::from_micros(1),
        None,
    )
    .unwrap();
    for i in 0..3 {
        log.note_receipt(
            "memory_search",
            "s1",
            "",
            0,
            &[],
            std::time::Duration::from_micros(i + 1),
        )
        .unwrap();
    }
    let st = log.stats().unwrap();
    assert_eq!((st.injections, st.empty_injections), (1, 0));
    assert_eq!(st.store_empty_ratio, Some(0.0));
    assert_eq!(st.injection_yield, Some(0.0), "查回来的是零条");
    assert_eq!(
        log.recent(20).unwrap().len(),
        4,
        "四行都在册：一行注入 + 三行回执（检索档不去重）"
    );
}

/// 只有回执、一次注入都没有 ⇒ 分母为空 = None，而不是 0.0 或除零 panic。
#[test]
fn yield_is_none_when_nothing_was_ever_injected() {
    let log = RecallLog::at(temp_dir("yield-none").join("usage.sqlite3"));
    log.note_receipt(
        "memory_search",
        "s1",
        "",
        1,
        &["a".to_string()],
        std::time::Duration::from_micros(1),
    )
    .unwrap();
    let st = log.stats().unwrap();
    assert_eq!(st.injections, 0);
    assert_eq!(st.store_empty_ratio, None);
    assert_eq!(st.injected_keys, 0);
    assert_eq!(st.injection_yield, None);
}
