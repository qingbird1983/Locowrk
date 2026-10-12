//! 整理事实卡两键（MR 稿 §5 提案 A · 后审计队列 Q3 · v2.186）的设置面：
//! 缺省值 / 磁盘往返 / 两字段互不牵连 / 出界拒绝。
//!
//! 从 `memory.rs` 单独立簇的理由 = 三问自检的第①问：那个文件管"记忆注入区"的
//! 既有键（topK / 尾巴策略 / collapse / 文件化…），本簇管的是**新增的整理副产物
//! 开关**，改卡的上限不会牵动注入档的读数，反之亦然。

use super::*;

/// M2 事实卡两键（MR 稿 §5 提案 A · 队列 Q3 · v2.186）：缺省 / 往返 / 出界。
///
/// 缺省是 `true` + 3，不是 `false`——这条要连着看 `settings/memory.rs` 里那段
/// 算术：整理本来就在跑、记录员本来就在读整段对话，多要 ≤3 张卡的边际成本只是
/// 一次调用的输出长度，而 `false` 会让"事实层永远 0 行"这个 §0-1 实证过的失效
/// 状态原地复现。开关的意义是**能关掉**，不是缺省关着。
#[test]
fn fact_card_switch_and_cap_default_roundtrip_and_validation() {
    let root = temp_root("fact-cards");
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().memory;
    assert!(v.fact_cards, "缺省开（理由见 settings/memory.rs 注释）");
    assert_eq!(v.fact_card_max, 3, "缺省 3 张");

    // 更新 + 磁盘往返
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                fact_cards: Some(false),
                fact_card_max: Some(7),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    let v = store.view().memory;
    assert!(!v.fact_cards);
    assert_eq!(v.fact_card_max, 7);
    let reopened = SettingsStore::open(&root).unwrap();
    assert!(!reopened.view().memory.fact_cards, "关掉必须落盘");
    assert_eq!(reopened.view().memory.fact_card_max, 7);

    // 两个字段互不牵连：开关回来不动上限
    store
        .update(
            None,
            None,
            None,
            None,
            None,
            None,
            Some(MemoryUpdate {
                fact_cards: Some(true),
                ..Default::default()
            }),
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    let v = store.view().memory;
    assert!(v.fact_cards);
    assert_eq!(v.fact_card_max, 7, "只改开关时上限保持不变");

    // 出界：区间是 1–10，0 与 11 都拒（0 该由开关表达而不是上限表达）
    for n in [0u32, 11] {
        let err = store
            .update(
                None,
                None,
                None,
                None,
                None,
                None,
                Some(MemoryUpdate {
                    fact_card_max: Some(n),
                    ..Default::default()
                }),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap_err();
        assert!(err.contains("事实卡上限"), "{n} 出界文案: {err}");
    }
    assert_eq!(
        store.view().memory.fact_card_max,
        7,
        "被拒的更新不许留下半改状态"
    );
    std::fs::remove_dir_all(&root).ok();
}
