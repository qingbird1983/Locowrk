//! 工具结果预算（v2.192 · 后审计队列 Q8 · CB §7.4）的设置面：缺省 / 旧文件
//! 字段级缺省 / 磁盘往返 / 出界拒绝 / 「0 = 不限制」这条口子是真的能关 / KiB→
//! 字节唯一换算处。
//!
//! 簇名不叫 `context`（被测子模块就叫 `settings/context.rs`）= 形状闸 S4 的硬
//! 约束：测试簇占住 `tests::context` 后，簇里的 `context::X` 会指向测试簇自己。
//! 从 `window.rs`（同区既有两键）单独立簇的理由 = 三问自检第①问：那文件管整理
//! 阈值与进度条的既有键，本簇管**新增的第三键**，改预算不牵动阈值读数。

use super::*;
// 两簇同一条路径（`settings::context::…` 随本批抬成 `pub(crate)`）：缺省档与换算都从
// 常量/函数那一份取，测试里不许手抄 24576 之类的数
use crate::settings::context::{tool_result_budget_bytes, TOOL_RESULT_BUDGET_DEFAULT_KIB};

// `set_context` 在刀⑥ 上收到 `settings/tests/mod.rs`（新簇要用同一枚包装），本簇
// 走 `use super::*` 拿它——形状闸拦"一个动作两个出处"，这里留一行指路。

/// 只改预算一个键。
fn set_budget(store: &SettingsStore, kib: u64) -> Result<(), String> {
    set_context(
        store,
        ContextUpdate {
            tool_result_budget_kib: Some(kib),
            ..Default::default()
        },
    )
}

#[test]
fn tool_result_budget_default_and_roundtrip() {
    let root = temp_root("budget-default");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省 = 24 KiB，且投影与后端 accessor 给的是同一个数（两个出处必须一致）。
    // 这里的 `24 * 1024` 是**刻意的独立对照**（哨兵），不是第二套算术：换算处若被
    // 改成 1000 进率，只有它会红——与 500KB 那条用例里的同款哨兵一个口径。
    assert_eq!(
        store.view().context.tool_result_budget_kib,
        TOOL_RESULT_BUDGET_DEFAULT_KIB
    );
    assert_eq!(store.tool_result_budget_bytes(), Some(24 * 1024));
    // 改档 + 磁盘往返（重开读回 = 永久保存的硬证明）
    set_budget(&store, 64).unwrap();
    assert_eq!(store.view().context.tool_result_budget_kib, 64);
    assert_eq!(
        SettingsStore::open(&root)
            .unwrap()
            .view()
            .context
            .tool_result_budget_kib,
        64
    );
    // 旧 settings.json 无此键 → 字段级缺省 24，**不许塌成 0**（塌成 0 等于
    // 老用户升级后这一层闸门静默消失，比缺省给高了还难发现）
    std::fs::write(
        root.join(FILE_NAME),
        r#"{"context":{"thresholdTokens":200000,"usageBar":false}}"#,
    )
    .unwrap();
    let v = SettingsStore::open(&root).unwrap().view().context;
    assert_eq!(v.tool_result_budget_kib, TOOL_RESULT_BUDGET_DEFAULT_KIB);
    assert_eq!(v.threshold_tokens, 200_000);
    assert!(!v.usage_bar);
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn tool_result_budget_validation_names_the_range() {
    let root = temp_root("budget-range");
    let store = SettingsStore::open(&root).unwrap();
    // 上限内 / 下限内 / 0（= 不限制）都收
    for ok in [0u64, 1, 24, 512] {
        set_budget(&store, ok).unwrap();
        assert_eq!(store.view().context.tool_result_budget_kib, ok);
    }
    // 出界拒绝，且**文案点名范围与 0 的含义**（只说「非法」用户不知道该填多少）
    for bad in [513u64, 1_024, 100_000] {
        let err = set_budget(&store, bad).unwrap_err();
        assert!(err.contains("工具结果预算"), "{err}");
        assert!(err.contains("0"), "{err}");
        assert!(err.contains("512"), "{err}");
        assert!(err.contains("不限制"), "{err}");
    }
    // 校验失败不得污染内存态（上一条成功值是 512）
    assert_eq!(store.view().context.tool_result_budget_kib, 512);
    // 三键互不牵连：改预算不动阈值与进度条，反之亦然
    let before = store.view().context;
    set_context(
        &store,
        ContextUpdate {
            threshold_tokens: Some(300_000),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(store.view().context.threshold_tokens, 300_000);
    assert_eq!(
        store.view().context.tool_result_budget_kib,
        before.tool_result_budget_kib
    );
    assert_eq!(store.view().context.usage_bar, before.usage_bar);
    std::fs::remove_dir_all(&root).ok();
}

/// KiB→字节只有一处算式（`settings/context.rs::tool_result_budget_bytes`）：
/// 组装点与 wire 都不许再乘 1024。这条把「0 = 不限制」与「非 0 = 该额度」两个
/// 分支同时钉住——写反一次，要么闸门永远不生效，要么用户关了也关不掉。
#[test]
fn tool_result_budget_bytes_conversion_is_the_single_source() {
    assert_eq!(tool_result_budget_bytes(0), None);
    assert_eq!(tool_result_budget_bytes(1), Some(1_024));
    assert_eq!(tool_result_budget_bytes(24), Some(24_576));
    assert_eq!(tool_result_budget_bytes(512), Some(524_288));
    // accessor 与换算处同源（不是各写一遍）
    let root = temp_root("budget-accessor");
    let store = SettingsStore::open(&root).unwrap();
    set_budget(&store, 0).unwrap();
    assert_eq!(store.tool_result_budget_bytes(), None);
    set_budget(&store, 96).unwrap();
    assert_eq!(
        store.tool_result_budget_bytes(),
        tool_result_budget_bytes(96)
    );
    std::fs::remove_dir_all(&root).ok();
}
