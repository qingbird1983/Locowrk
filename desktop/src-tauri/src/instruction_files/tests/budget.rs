use super::*;

// 链预算判据（v2.188 · Q4；S4 刀③c 改为写死单档）：截断标注、挤掉点名、
// 两条链各自满额。
// ⚠ 内容字符只用 z / q（与截断、跳过两条标记句和 # agentsMd 头部都不撞名），
// 所以「按字符计数」能当精确读数用；既有那条用 x 的判据口径不同，别混用。

/// 项目链预算钳在**第二个族**上：第一个族全量、第二个族截到剩余额度，
/// 标记句报的是"还剩多少"（报整条链的档位就是在说谎）。
#[test]
fn project_budget_clamps_the_second_family_and_marks_the_leftover_room() {
    let (data, repo, ws) = fixture("chain-clamp");
    std::fs::write(repo.join("AGENTS.md"), "z".repeat(3_000)).unwrap();
    std::fs::write(repo.join("CLAUDE.md"), "q".repeat(9_000)).unwrap();
    // 8 KiB = 8192 字节：AGENTS 吃掉 3000，CLAUDE 只剩 5192 ⇒ 5192/1024 = 5 KB
    let body = source_with_cap(&data, &ws, 8)
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
/// ⚠ 这条不是重复覆盖——「标记报整条链额度」那个变异在 room == cap 时
/// 报出来的数字**恰好也对**，只有 room 与 cap 分开的夹具能把它区分开。
#[test]
fn a_budget_shrinking_still_clamps_the_project_chain() {
    let (data, repo, ws) = fixture("chain-shrink");
    std::fs::write(repo.join("AGENTS.md"), "z".repeat(2_048)).unwrap();
    std::fs::write(repo.join("CLAUDE.md"), "q".repeat(4_000)).unwrap();
    // 4 KiB = 4096：AGENTS 吃掉 2048 ⇒ CLAUDE 只剩 2048 = **2 KB**，而链档位是 4 KB
    let body = source_with_cap(&data, &ws, 4)
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
    let (data, repo, ws) = fixture("chain-skip");
    std::fs::write(repo.join("AGENTS.md"), "z".repeat(4_096)).unwrap();
    std::fs::write(repo.join("CLAUDE.md"), "q".repeat(10)).unwrap();
    let body = source_with_cap(&data, &ws, 4)
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

/// 两条链**各自吃满同一额度**，不共享计数器：全局 9000 字节与项目 9000 字节
/// 在同一档 4 KiB 下都被截到 4096，说明第二条链没有从第一条链的已用额度里扣。
/// （改前这是"两档独立旋钮"的用例；S4 刀③c 只剩一个档位后，它证的是
/// "一个档位作用两次、互不侵占"，共用计数器会把项目链压成 0。）
#[test]
fn global_and_project_chains_each_get_the_full_cap() {
    let (data, repo, ws) = fixture("two-chains");
    std::fs::write(data.join("AGENTS.md"), "z".repeat(9_000)).unwrap();
    std::fs::write(repo.join("AGENTS.md"), "q".repeat(9_000)).unwrap();

    let body = source_with_cap(&data, &ws, 4)
        .load(Some("ws-session"))
        .unwrap();
    assert_eq!(body.matches('z').count(), 4_096, "全局链钳在 4 KiB");
    assert_eq!(
        body.matches('q').count(),
        4_096,
        "项目链另有 4 KiB，不被全局挤压"
    );
    let _ = std::fs::remove_dir_all(&data);
}

/// 生产档位的对账：写死值必须是 **100 KiB**——旧设置项缺省是 100，动一个数，
/// 「只有一份 AGENTS.md」这个存量主形态的注入字节就跟着动（AC-7）。
#[test]
fn production_cap_is_the_old_hardcoded_100_kib() {
    assert_eq!(
        crate::instruction_files::INSTRUCTION_CHAIN_KIB,
        100,
        "写死预算 = 改前那颗写死/缺省值"
    );
}

/// 阳性对照：**没有任何候选文件**时预算不得造出内容（否则链装配体的
/// 「空 = None」早退被破，零负担档变成注入空段）。
#[test]
fn budgets_never_invent_a_section_when_no_file_exists() {
    let (data, _repo, ws) = fixture("budget-empty");
    for cap in [8usize, 100] {
        assert!(
            source_with_cap(&data, &ws, cap)
                .load(Some("ws-session"))
                .is_none(),
            "缺省零负担：档位 {cap} KiB 不该造出段"
        );
    }
    let _ = std::fs::remove_dir_all(&data);
}

// S4 刀③c：`a_relocked_budget_does_not_move_an_ongoing_session` 随"两档预算"删除。
// 预算不再可热调，就没有"中途改预算"这件事可测；快照锁字节的纪律本身仍由
// `legacy::snapshot_locks_per_session`（同会话不重读文件）钉住。
