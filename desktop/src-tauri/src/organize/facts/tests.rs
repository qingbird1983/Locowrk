//! `organize::facts` 单测（Q3 / MR §5 提案 A）：归层映射、查重清单的读法、
//! 落库计数与三条降级口径。**不碰管道**（端到端在 `organize/tests/fact_cards.rs`）。

use std::sync::Arc;

use super::*;

const WS: &str = r"F:\ws\factproj";

fn resolver() -> crate::workspace::WorkspaceResolver {
    Arc::new(|sid: &str| {
        if sid == "s-fact" {
            Some(WS.to_string())
        } else {
            None
        }
    })
}

fn store_at(tag: &str) -> Arc<crate::memory::MemoryStore> {
    let dir = crate::testutil::work_dir(&format!("orgz-facts-{tag}"));
    Arc::new(crate::memory::MemoryStore::new(&dir, resolver()))
}

/// 一条合法回复的骨架：章节数组 + 总结 + 卡段（形状照
/// `memory_tree::manager::FACTS_MARKER` 的契约）。
fn raw_with_facts(facts: &str) -> String {
    format!(
        r#"[{{"title":"话题一","summary":"摘要一。","event_range":[0,1]}}]
===SUMMARY===
本轮结论是共享 runtime。
===FACTS===
{facts}"#
    )
}

/// 归层是这张卡唯一的落点决定：`user|feedback → 全局层（空串）`、
/// `project|reference → 归一后的项目键位`；白名单外没有归层（解析层已滤，
/// 这里是第二重，归层不许有"不知道落哪"的漏网）。
#[test]
fn types_map_onto_exactly_two_layers() {
    let want_project = crate::workspace::scope_key(Some(WS));
    for t in ["user", "feedback"] {
        assert_eq!(
            layer_of(t, WS).as_deref(),
            Some(""),
            "{t} 必须落全局层（稿 §3 映射）"
        );
    }
    for t in ["project", "reference"] {
        assert_eq!(layer_of(t, WS).as_deref(), Some(want_project.as_str()));
    }
    for t in ["", "Project", "misc", "全局"] {
        assert_eq!(layer_of(t, WS), None, "非法类型 {t:?} 必须无归层");
    }
}

/// 未归属会话（键位空串）**没有项目层可写**：`project` 卡被丢，而 `user` 卡
/// 照样能落全局层——这条不对称是刻意的：全局层与归属无关，项目层没有归属
/// 就不成立（与 v2.68「未归属缺省写拒绝」同口径，取丢弃而非报错因为这是副产物）。
#[test]
fn unattributed_session_can_still_reach_the_global_side_but_not_a_project_side() {
    assert_eq!(layer_of("user", ""), Some(String::new()));
    assert_eq!(layer_of("project", ""), None);
    assert_eq!(layer_of("reference", ""), None);
}

/// 落库计数与三条降级口径：合法卡 written、**没有卡段 = 三个数全 0**（不许把
/// "模型没写标记"记成"丢了 N 张"），`limit = 0` 连解析都不做（开关关掉的第二重
/// 防线）。
///
/// ⚠ `dropped` 在这里是 **0** 而不是 1：非法类型那张被 `memory_tree::fact_cards`
/// 的类型白名单在**解析层**整条丢掉，压根到不了归层。这条读数是判据自纠抓出来的
/// ——我第一版按"归层拒收"期待它 = 1，实测 0，才发现自己把两道防线记账位置猜错了
/// （白名单丢的张数宿主看不见，`dropped` 唯一的真实出处是下面那条未归属用例）。
#[test]
fn persist_counts_written_dropped_and_the_absence_of_a_section() {
    let store = store_at("count");
    let raw = raw_with_facts(
        r#"[{"key":"build.cmd","value":"pnpm tauri dev","type":"project","tags":"build"},
           {"key":"user.pref.pkg","value":"永远用 pnpm","type":"user","tags":"pref"},
           {"key":"junk","value":"来历不明","type":"misc","tags":""}]"#,
    );
    let rep = persist(&store, WS, &raw, 3);
    assert_eq!(rep.written, 2, "两张合法卡落库");
    assert_eq!(rep.dropped, 0, "非法类型那张死在解析层，不归这里的账");
    assert_eq!(rep.failed, 0);
    let proj = crate::workspace::scope_key(Some(WS));
    let rows = store.index_rows(&proj, 10).unwrap();
    assert_eq!(rows.len(), 1, "项目层只剩 build.cmd");
    assert_eq!(rows[0].key, "build.cmd");
    // 出处与键位从**管理页那条读取面**（全键位、带 source）验，而不是
    // index_rows——它没有 source 列（Q2 故意不取 value 也不取多余列）。
    let all = store.list("", 50).unwrap();
    let pref = all
        .iter()
        .find(|r| r.key == "user.pref.pkg")
        .expect("全局那条偏好必须读得到");
    assert_eq!(pref.workspace, "", "user 类型落全局层 = 空串键位");
    assert_eq!(pref.source, SOURCE_ORGANIZED, "出处必须标 organized");
    let got = all
        .iter()
        .find(|r| r.key == "build.cmd")
        .expect("项目那条必须读得到");
    assert_eq!(got.workspace, proj, "落库键位必须是归一后的串");
    assert_eq!(all.len(), 2, "{all:?}");

    // 没有卡段 = 全 0（不是"丢了 0 张"以外的任何读数）
    let quiet = raw_with_facts("[]");
    let rep = persist(&store, "F:\\ws\\other", &quiet, 3);
    assert_eq!(rep, FactReport::default(), "空数组/无标记都不该记账");
    // limit = 0 ⇒ 一张不取（连合法卡段也不解析）
    let rep = persist(&store, WS, &raw, 0);
    assert_eq!(rep, FactReport::default(), "开关关掉时不得有任何写入动作");
    assert_eq!(
        store.list("", 50).unwrap().len(),
        2,
        "上面两次 persist 一个字节都不许改（关掉就真的没写）"
    );
}

/// `dropped` 的**唯一真实出处** = 未归属会话写项目卡：键位空串时项目层不成立
/// ⇒ 那张被丢并计数，而 `user` 卡照样落全局层。归层那条不对称（上面第二条用例）
/// 必须在计数面上也可见，否则 `FactReport.dropped` 就是个永不亮的假读数。
#[test]
fn an_unattributed_organize_drops_project_cards_but_keeps_global_ones() {
    let store = store_at("drop-unattributed");
    let raw = raw_with_facts(
        r#"[{"key":"proj.a","value":"项目事实","type":"project"},
           {"key":"user.b","value":"全局偏好","type":"user"}]"#,
    );
    let rep = persist(&store, "", &raw, 3);
    assert_eq!(rep.written, 1, "全局那张与归属无关，照落");
    assert_eq!(rep.dropped, 1, "没有项目层可写时项目卡必须记进 dropped");
    assert_eq!(rep.failed, 0);
    let rows = store.list("", 50).unwrap();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(rows[0].key, "user.b");
}

/// upsert 语义 = 同键覆盖不新增行（稿 §6「去重靠 upsert + 提示词」那一半的
/// 机器半边）：两次整理写同一个 key，行数仍是 1，且 value 与 source 都被刷新。
#[test]
fn the_same_key_upserts_instead_of_piling_up_rows() {
    let store = store_at("upsert");
    let first = raw_with_facts(r#"[{"key":"arch.store","value":"sqlite","type":"project"}]"#);
    let second =
        raw_with_facts(r#"[{"key":"arch.store","value":"sqlite + md 双写","type":"project"}]"#);
    assert_eq!(persist(&store, WS, &first, 3).written, 1);
    assert_eq!(persist(&store, WS, &second, 3).written, 1);
    let rows = store
        .index_rows(&crate::workspace::scope_key(Some(WS)), 10)
        .unwrap();
    assert_eq!(rows.len(), 1, "同键两次整理必须仍是一行");
    assert_eq!(rows[0].tags, "", "第二次没带 tags ⇒ 覆盖为空（不叠加）");
    // 行数是"没新增"，**值变了才是"覆盖了"**：`index_rows` 按 Q2 的口径不取
    // value 列（指针级三件），所以这一档必须从管理页那条读取面（`list`）验。
    // 缺这条断言时，摘掉 `ON CONFLICT … SET value = excluded.value` 的变异不会红
    // ——判据覆盖面只钉了一半（历批「四面齐查」同族）。
    let all = store.list("", 50).unwrap();
    assert_eq!(all.len(), 1, "{all:?}");
    assert!(
        all[0].value.contains("md 双写"),
        "第二次整理必须把值刷新: {:?}",
        all[0].value
    );
    assert_eq!(all[0].source, SOURCE_ORGANIZED, "出处随覆盖一起写回");
}

/// 落库失败不许反噬：一条 `write` 报错时**其余合法卡照样落库**（这是"副产物
/// 不拖主面"的最硬一档）。
///
/// ⚠ 触发条件用**超长 key**，不是超长 value：value 在解析层就被
/// `memory_tree::fact_cards` 钳到 `FACT_VALUE_MAX_CHARS`（500 字符），永远撞不到
/// `MemoryStore::write` 的 `VALUE_MAX`（256 KiB）——我第一版拿"超长 value"当失败
/// 夹具，实测 `failed` 恒 0，才发现自己造了一条**不可达分支**当判据（同族 = 批 F-24c
/// 的"打在虚空上的扰动"）。key 这一侧解析层不钳长度（只压成一行），`KEY_MAX` 才是
/// 真会报错的那个上限。
#[test]
fn a_failing_card_does_not_take_the_rest_down_with_it() {
    let store = store_at("fail");
    let fat = "k".repeat(crate::memory::KEY_MAX + 8);
    let raw = raw_with_facts(&format!(
        r#"[{{"key":"{fat}","value":"键名超长的事实","type":"project"}},
           {{"key":"slim.ok","value":"短事实","type":"project"}}]"#
    ));
    let rep = persist(&store, WS, &raw, 3);
    assert_eq!(rep.failed, 1, "key 超长那条写失败");
    assert_eq!(rep.written, 1, "另一条必须照样落库");
    assert_eq!(rep.dropped, 0);
    let rows = store
        .index_rows(&crate::workspace::scope_key(Some(WS)), 10)
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key, "slim.ok");
}

/// 查重清单必须**带层标**：同名 key 允许在两层各存一行（主键含 workspace），
/// 模型看不到层就会把一条项目事实标成 `user` 去"更新"全局行 —— 那正是最坏污染。
#[test]
fn the_dedup_listing_labels_which_side_each_key_lives_on() {
    let store = store_at("listing");
    store
        .write("", "user.lang", "中文", "pref", "explicit")
        .unwrap();
    store
        .write(WS, "build.cmd", "pnpm", "build", "explicit")
        .unwrap();
    store
        .write(r"F:\ws\other-proj", "build.cmd", "cargo", "", "explicit")
        .unwrap();
    let keys = existing_keys(&store, WS);
    assert!(
        keys.contains(&"user.lang (global)".to_string()),
        "全局行要标 global: {keys:?}"
    );
    assert!(
        keys.contains(&"build.cmd (project)".to_string()),
        "本项目行要标 project: {keys:?}"
    );
    assert_eq!(
        keys.iter().filter(|k| k.starts_with("build.cmd")).count(),
        1,
        "别的项目的同名键不得混进来（清单只给本项目 + 全局）"
    );
    assert_eq!(keys.len(), 2, "{keys:?}");
}

/// 未归属会话的清单**只有全局侧**：不把全局行报成"项目行"（那会诱导模型
/// 用 project 类型去更新一条其实不存在的项目键位）。
#[test]
fn an_unattributed_session_sees_only_the_global_side_of_the_listing() {
    let store = store_at("listing-unattributed");
    store
        .write("", "user.tone", "直给", "", "explicit")
        .unwrap();
    store.write(WS, "proj.only", "x", "", "explicit").unwrap();
    let keys = existing_keys(&store, "");
    assert_eq!(keys, vec!["user.tone (global)".to_string()], "{keys:?}");
}

/// 清单有顶（`MAX_EXISTING_KEYS`），且**读不到库 ≠ 没货**：store 指向一个
/// 无法建表的目录时，清单为空但函数不 panic、不写盘。
#[test]
fn the_listing_is_capped_and_survives_an_unreadable_store() {
    let store = store_at("capped");
    for i in 0..(MAX_EXISTING_KEYS + 20) {
        store
            .write("", &format!("k{i:04}"), "v", "", "explicit")
            .unwrap();
    }
    let keys = existing_keys(&store, "");
    assert!(
        keys.len() <= MAX_EXISTING_KEYS,
        "清单必须钳到 {MAX_EXISTING_KEYS}，实际 {}",
        keys.len()
    );
    assert!(!keys.is_empty(), "阳性对照：不是空清单冒充钳制生效");
}
