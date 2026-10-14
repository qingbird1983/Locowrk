//! 队列 Q9 学习档的**设置面**用例（v2.200）：缺省开 / 四档桥接到生效面 /
//! 部分更新语义 / 五键进投影 / 出界点名。
//!
//! 为什么单独一簇而不是塞进 `settings/tests/memory.rs`：那件在 HEAD 是 386 行，
//! 学习档的这五段再加进去就同时管「既有记忆区形状」与「本刀新增档」两件事，
//! 而且必然越过 400 行闸（`check_length` 基线 22 ⇒ 处置是拆簇，不是抬基线、
//! 也不是压注释凑行数）。既有那两笔 `MemoryUpdate` 字面量只补了 5 个 `None`
//! 字段——**显式列字段**是这簇有意保持的性质：新增设置项必须被显式考虑过。

use super::*;

/// 缺省 = 学习档**开**（对照档要用户显式选），且四档从设置面原样搬到生效面。
///
/// ⚠ 测试里不手抄 400/250/200/150：数字全部取自内核 `LearnWeights::default`，
/// 否则"设置面显示 400、打分面用 350"这种漂移测不出来（缺省值的唯一真源 =
/// 内核 `LEARN_WEIGHT_DEFAULT` 那一个数组，形状判据 S6 钉它）。
#[test]
fn learn_defaults_are_on_and_bridge_to_the_effective_config() {
    let root = temp_root("learn-defaults");
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().memory;
    let cfg = store.memory_inject_config();
    let d = kernel_agent::LearnWeights::default();
    assert!(v.learn_enabled);
    assert_eq!(
        (
            v.relevance_weight,
            v.strength_weight,
            v.freshness_weight,
            v.recency_weight
        ),
        (d.relevance, d.strength, d.freshness, d.recency)
    );
    assert_eq!(
        (
            cfg.learn.relevance,
            cfg.learn.strength,
            cfg.learn.freshness,
            cfg.learn.recency
        ),
        (
            v.relevance_weight,
            v.strength_weight,
            v.freshness_weight,
            v.recency_weight
        )
    );
    assert!(cfg.learn.enabled);
    std::fs::remove_dir_all(&root).ok();
}

/// 部分更新语义：只给三档，另两档传 `None` 必须保持缺省（与既有各区同一条规矩）。
#[test]
fn learn_weights_roundtrip_keep_unspecified_dials() {
    let root = temp_root("learn-roundtrip");
    let store = SettingsStore::open(&root).unwrap();
    set_memory(
        &store,
        MemoryUpdate {
            learn_enabled: Some(false),
            relevance_weight: Some(700),
            strength_weight: None,
            freshness_weight: None,
            recency_weight: Some(300),
            ..Default::default()
        },
    )
    .unwrap();
    drop(store);
    let v = SettingsStore::open(&root).unwrap().view().memory;
    let d = kernel_agent::LearnWeights::default();
    assert!(!v.learn_enabled);
    assert_eq!(v.relevance_weight, 700);
    assert_eq!(v.recency_weight, 300);
    assert_eq!(v.strength_weight, d.strength);
    assert_eq!(v.freshness_weight, d.freshness);
    std::fs::remove_dir_all(&root).ok();
}

/// 五键必须进投影：管理页与设置页读的都是 view 面，少一个键不是"少显示一个数字",
/// 是那条控件**根本不存在**（前端拿 undefined 填滑条）。
#[test]
fn learn_keys_are_in_the_projection() {
    let root = temp_root("learn-projection");
    let v = SettingsStore::open(&root).unwrap().view().memory;
    let json = serde_json::to_value(&v)
        .unwrap()
        .as_object()
        .unwrap()
        .clone();
    for key in [
        "learnEnabled",
        "relevanceWeight",
        "strengthWeight",
        "freshnessWeight",
        "recencyWeight",
    ] {
        assert!(json.contains_key(key), "Q9 新键 {key} 必须进投影");
    }
    std::fs::remove_dir_all(&root).ok();
}

/// 校验面：**出界点名是哪一档**，且「全 0」与「四档和不为 1000」都放行。
/// 本刀刻意不设"和为 1000"这条硬约束（理由见 `settings/memory.rs::LEARN_WEIGHT_MAX`
/// 头注：排序只看相对大小，硬约束会逼出一个联动机制）。这条用例存在的意义就是把
/// "将来有人觉得不严谨、顺手加上和校验"挡住——加上之后全 0 会被拒，而那正是
/// 现役行为的实验档。
#[test]
fn learn_weight_validation_accepts_any_sum_but_rejects_out_of_range() {
    let root = temp_root("learn-weight");
    let store = SettingsStore::open(&root).unwrap();
    let dials: [fn(&mut MemoryUpdate); 4] = [
        |u| u.relevance_weight = Some(1001),
        |u| u.strength_weight = Some(65_535),
        |u| u.freshness_weight = Some(1001),
        |u| u.recency_weight = Some(1001),
    ];
    for (dial, label) in dials.iter().zip(["相关性", "强度", "热度", "新鲜度"]) {
        let mut u = MemoryUpdate::default();
        dial(&mut u);
        let err = set_memory(&store, u).unwrap_err();
        assert!(err.contains(label), "{label} 出界必须点名，实得：{err}");
    }
    // 放行：全 0（= 四信号都不贡献 ⇒ 排序退化为现役 tie-break）
    set_memory(
        &store,
        MemoryUpdate {
            relevance_weight: Some(0),
            strength_weight: Some(0),
            freshness_weight: Some(0),
            recency_weight: Some(0),
            ..Default::default()
        },
    )
    .unwrap();
    let v = store.view().memory;
    assert_eq!(
        (
            v.relevance_weight,
            v.strength_weight,
            v.freshness_weight,
            v.recency_weight
        ),
        (0, 0, 0, 0),
        "全 0 是合法档，不得被和校验吞掉"
    );
    // 放行：和 = 700（不为 1000）
    set_memory(
        &store,
        MemoryUpdate {
            relevance_weight: Some(700),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(store.view().memory.relevance_weight, 700);
    std::fs::remove_dir_all(&root).ok();
}
