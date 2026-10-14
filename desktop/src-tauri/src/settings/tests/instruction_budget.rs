//! 指令文件两档链预算（v2.188 · 后审计队列 Q4）的设置面：缺省 / 磁盘往返 /
//! 两档互不牵连 / 出界拒绝 / 出界文案各自点名。
//!
//! 从 `memory.rs` 单独立簇的理由 = 三问自检第①问（与 `fact_cards.rs` 同判）：
//! 那文件管记忆注入区的既有键，本簇管**新增的两条链额度**，改额度不牵动注入档读数。

use super::*;

/// 两档缺省都是 **100 KiB**（= 改前那颗写死的 `MAX_FILE_BYTES`）、区间 8–512、
/// 往返落盘、两档互不牵连、出界拒。
#[test]
fn instruction_chain_budgets_default_roundtrip_and_validation() {
    let root = temp_root("instruction-budget");
    let store = SettingsStore::open(&root).unwrap();
    let v = store.view().memory;
    assert_eq!(v.instruction_global_kib, 100, "全局链缺省 = 改前写死值");
    assert_eq!(v.instruction_project_kib, 100, "项目链缺省 = 改前写死值");

    set_memory(
        &store,
        MemoryUpdate {
            instruction_global_kib: Some(16),
            instruction_project_kib: Some(24),
            ..Default::default()
        },
    )
    .unwrap();
    let v = store.view().memory;
    assert_eq!(
        (v.instruction_global_kib, v.instruction_project_kib),
        (16, 24)
    );
    let reopened = SettingsStore::open(&root).unwrap();
    assert_eq!(
        reopened.view().memory.instruction_global_kib,
        16,
        "改了必须落盘"
    );
    assert_eq!(reopened.view().memory.instruction_project_kib, 24);

    // 两档互不牵连：只调项目链时全局链保持原值（合并成一档的实现会在这里红）
    set_memory(
        &store,
        MemoryUpdate {
            instruction_project_kib: Some(64),
            ..Default::default()
        },
    )
    .unwrap();
    let v = store.view().memory;
    assert_eq!(v.instruction_global_kib, 16, "只改项目链不动全局链");
    assert_eq!(v.instruction_project_kib, 64);

    // 端点合法（8 与 512）：范围写成开区间会把两个可用值拒掉
    for n in [8u32, 512] {
        set_memory(
            &store,
            MemoryUpdate {
                instruction_global_kib: Some(n),
                ..Default::default()
            },
        )
        .unwrap_or_else(|e| panic!("{n} 是合法端点: {e}"));
        assert_eq!(store.view().memory.instruction_global_kib, n);
    }

    // 出界：区间外两侧都拒，且被拒的更新不许留下半改状态
    for n in [7u32, 513] {
        let err = set_memory(
            &store,
            MemoryUpdate {
                instruction_global_kib: Some(n),
                instruction_project_kib: Some(n),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("超出范围"), "{n} 出界文案: {err}");
    }
    assert_eq!(
        store.view().memory.instruction_global_kib,
        512,
        "被拒的更新回留在最后一次成功值上"
    );
    std::fs::remove_dir_all(&root).unwrap();
}

/// 两档越界必须**各自点名**：文案写成一条通用句（或两处标签复制成同一个），
/// 用户在界面上就看到"项目链超范围"却改的是全局链——排障方向直接反掉。
#[test]
fn each_out_of_range_dial_names_itself_in_the_error() {
    let root = temp_root("instruction-budget-labels");
    let store = SettingsStore::open(&root).unwrap();

    let err = set_memory(
        &store,
        MemoryUpdate {
            instruction_global_kib: Some(513),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(err.contains("全局指令链预算"), "全局档文案: {err}");
    assert!(
        !err.contains("项目指令链预算"),
        "全局档越界不许报项目：{err}"
    );

    let err = set_memory(
        &store,
        MemoryUpdate {
            instruction_project_kib: Some(7),
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(err.contains("项目指令链预算"), "项目档文案: {err}");
    assert!(
        !err.contains("全局指令链预算"),
        "项目档越界不许报全局：{err}"
    );

    // 端点写进文案（不是"范围见文档"）：界面报错要能自解释
    assert!(err.contains("8–512 KiB"), "区间端点必须在文案里：{err}");
    std::fs::remove_dir_all(&root).unwrap();
}
