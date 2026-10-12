use super::*;

// 链预算判据（v2.188 · Q4）：两档独立、剩余额度标注、挤掉点名、快照锁字节覆盖预算。
// ⚠ 内容字符只用 z / q（与截断、跳过两条标记句和 # agentsMd 头部都不撞名），
// 所以「按字符计数」能当精确读数用；既有那条用 x 的判据口径不同，别混用。

/// 项目链预算钳在**第二个族**上：第一个族全量、第二个族截到剩余额度，
/// 标记句报的是"还剩多少"（报整条链的档位就是在说谎）。
#[test]
fn project_budget_clamps_the_second_family_and_marks_the_leftover_room() {
    let (data, repo, store) = fixture("chain-clamp");
    std::fs::write(repo.join("AGENTS.md"), "z".repeat(3_000)).unwrap();
    std::fs::write(repo.join("CLAUDE.md"), "q".repeat(9_000)).unwrap();
    // 8 KiB = 8192 字节：AGENTS 吃掉 3000，CLAUDE 只剩 5192 ⇒ 5192/1024 = 5 KB
    let body = source_budgeted(&data, &store, true, 100, 8)
        .load(Some("ws-session"))
        .unwrap();
    assert_eq!(body.matches('z').count(), 3_000, "第一个族不被压");
    assert_eq!(body.matches('q').count(), 5_192, "第二个族钳到剩余额度");
    assert!(
        body.contains("[File truncated: only 5 KB of the instruction chain budget was left]"),
        "标记带的是这个文件实际拿到的额度：{body}"
    );
    let _ = std::fs::remove_dir_all(&data);
}

/// 额度**收缩但没归零**时的截断标注：这一档和上一条的区别是 `room != cap`
/// （上一条里第二个族恰好从链首开始用，room 就等于 cap）。
/// ⚠ 这条不是重复覆盖——「标记报整条链额度」那个变异（M4）在 room == cap 时
/// 报出来的数字**恰好也对**，只有 room 与 cap 分开的夹具能把它区分开；缺了这条，
/// M4 就只能咬到整链读数那半边，标记语义这半边是空转判据。
#[test]
fn a_budget_shrinking_still_clamps_the_project_chain() {
    let (data, repo, store) = fixture("chain-shrink");
    std::fs::write(repo.join("AGENTS.md"), "z".repeat(2_048)).unwrap();
    std::fs::write(repo.join("CLAUDE.md"), "q".repeat(4_000)).unwrap();
    // 4 KiB = 4096：AGENTS 吃掉 2048 ⇒ CLAUDE 只剩 2048 = **2 KB**，而链档位是 4 KB
    let body = source_budgeted(&data, &store, true, 100, 4)
        .load(Some("ws-session"))
        .unwrap();
    assert_eq!(body.matches('z').count(), 2_048, "第一个族不被压");
    assert_eq!(body.matches('q').count(), 2_048, "第二个族钳到剩余额度");
    assert!(
        body.contains("[File truncated: only 2 KB of the instruction chain budget was left]"),
        "标记带剩余额度而不是链档位：{body}"
    );
    assert!(
        !body.contains("[File truncated: only 4 KB"),
        "4 KB 是整条链的档位，这个文件只拿到 2 KB"
    );
    let _ = std::fs::remove_dir_all(&data);
}

/// 额度归零 = 后面的族**不读**，但必须点名说出来（静默少注一族的约定，
/// 模型会以为自己拿到了全部项目规则）。
#[test]
fn exhausted_budget_skips_later_families_but_names_them() {
    let (data, repo, store) = fixture("chain-skip");
    std::fs::write(repo.join("AGENTS.md"), "z".repeat(4_096)).unwrap();
    std::fs::write(repo.join("CLAUDE.md"), "q".repeat(10)).unwrap();
    let body = source_budgeted(&data, &store, true, 100, 4)
        .load(Some("ws-session"))
        .unwrap();
    assert_eq!(body.matches('z').count(), 4_096, "刚好装满，不截断");
    assert_eq!(body.matches('q').count(), 0, "第二族整族不读");
    let claude = repo.join("CLAUDE.md").display().to_string();
    assert!(
        body.contains(&format!(
            "[Skipped {claude}: the instruction chain budget of 4 KB is already exhausted]"
        )),
        "跳过要点名：{body}"
    );
    assert!(
        !body.contains(&format!("Contents of {claude}")),
        "被跳过的族不留段头"
    );
    let _ = std::fs::remove_dir_all(&data);
}

/// 两档是**两个独立旋钮**，双向都验：全局被压不许挤掉项目，项目被压也不许挤掉全局。
/// 只验一个方向的夹具抓不住"两条链合用一个额度"的实现——那恰好只在另一个方向露馅。
#[test]
fn global_and_project_budgets_do_not_press_each_other() {
    let (data, repo, store) = fixture("two-dials");
    std::fs::write(data.join("AGENTS.md"), "z".repeat(9_000)).unwrap();
    std::fs::write(repo.join("AGENTS.md"), "q".repeat(9_000)).unwrap();

    // 全局压到 4 KiB，项目留 100 KiB
    let body = source_budgeted(&data, &store, true, 4, 100)
        .load(Some("ws-a"))
        .unwrap();
    assert_eq!(body.matches('z').count(), 4_096, "全局链钳在 4 KiB");
    assert_eq!(body.matches('q').count(), 9_000, "项目链不受全局档位挤压");

    // 反向：项目压到 4 KiB，全局留 100 KiB
    let body = source_budgeted(&data, &store, true, 100, 4)
        .load(Some("ws-b"))
        .unwrap();
    assert_eq!(body.matches('z').count(), 9_000, "全局链不受项目档位挤压");
    assert_eq!(body.matches('q').count(), 4_096, "项目链钳在 4 KiB");
    let _ = std::fs::remove_dir_all(&data);
}

/// 缺省档位的对账：两档都必须是 **100**——那颗写死的 `MAX_FILE_BYTES` 换成可调项时，
/// 缺省值一动，「只有一份 AGENTS.md」这个存量主形态的注入字节就跟着动。
/// ⚠ 这里走 `MemorySettings::default()` 而不是直接引 `INSTRUCTION_*_DEFAULT` 常量：
/// 后者在私有 `settings::memory` 里，为了一条测试去抬生产可见性 = 谎报 API 面。
/// 合法区间 8–512 的端点由设置面自己的出界判据钉（`settings/tests/instruction_budget.rs`）。
#[test]
fn default_budgets_equal_the_old_hardcoded_cap() {
    let d = crate::settings::MemorySettings::default();
    assert_eq!(d.instruction_global_kib, 100, "全局链缺省 = 改前那颗写死值");
    assert_eq!(
        d.instruction_project_kib, 100,
        "项目链缺省 = 改前那颗写死值"
    );
}

/// 预算档位**同样吃快照锁字节**：中途改设置不许改写已定格的会话（消息字节 = 前缀）。
/// 双向都有独有点——摘掉缓存 ⇒ 同会话第二次跟着变小；预算没接进读盘路径 ⇒ 新会话
/// 仍按旧额度。
#[test]
fn a_relocked_budget_does_not_move_an_ongoing_session() {
    use std::sync::atomic::{AtomicU32, Ordering};
    let (data, repo, store) = fixture("budget-snapshot");
    std::fs::write(repo.join("AGENTS.md"), "z".repeat(9_216)).unwrap();
    let budget = Arc::new(AtomicU32::new(100));
    let probe = Arc::clone(&budget);
    let src = InstructionFiles::new(
        Arc::clone(&store),
        data.join("AGENTS.md"),
        Arc::new(|| true) as Arc<dyn Fn() -> bool + Send + Sync>,
        Arc::new(move || {
            let v = probe.load(Ordering::SeqCst);
            (v, v)
        }) as Arc<dyn Fn() -> (u32, u32) + Send + Sync>,
    );
    let first = src.load(Some("ws-session")).unwrap();
    assert_eq!(
        first.matches('z').count(),
        9_216,
        "首查按当时的 100 KiB 全量读"
    );

    budget.store(8, Ordering::SeqCst);
    assert_eq!(
        src.load(Some("ws-session")).unwrap(),
        first,
        "已定格的会话不吃新预算（改了额度也不重锁消息前缀）"
    );
    let fresh = src.load(Some("ws-fresh")).unwrap();
    assert_eq!(fresh.matches('z').count(), 8 * 1024, "新会话按新额度");
    assert!(fresh.contains("[File truncated: only 8 KB"), "{fresh}");
    let _ = std::fs::remove_dir_all(&data);
}

/// 阳性对照：**没有任何候选文件**时两档预算都不得造出内容（否则链装配体的
/// 「空 = None」早退被破，零负担档变成注入空段）。
#[test]
fn budgets_never_invent_a_section_when_no_file_exists() {
    let (data, _repo, store) = fixture("budget-empty");
    for (g, p) in [(8, 8), (512, 512), (100, 8)] {
        assert!(
            source_budgeted(&data, &store, true, g, p)
                .load(Some("ws-session"))
                .is_none(),
            "缺省零负担：档位 {g}/{p} 不该造出段"
        );
    }
    let _ = std::fs::remove_dir_all(&data);
}
