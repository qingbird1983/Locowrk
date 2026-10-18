//! 读回面（自 `tests.rs` 搬入，用例本体逐字未改；S1 刀②a 立）：按键位取一条与扫全根
//! 两种查法，加上"旧格式照样读"那一档——真源翻成文件之后，读不出来的东西就是没有，
//! 所以这片必须把**"没有"与"读不动"分开**。

use super::*;

/// tags 必须是**读得回的一等字段**：`description` 是"tags，空则正文首行"的有损
/// hook，从它反推不出"这串是标签还是正文第一句"。真源 = 文件之后，管理页过滤与
/// memory_search 的 tags 匹配只能从 `tags:` 这行取（设计 §2.1）。
#[test]
fn tags_survive_the_write_read_roundtrip_as_a_first_class_field() {
    let root = std::env::temp_dir().join(format!("memlayout-tags-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let ws = r"F:\proj\Tags";
    let mut e = entry(ws, "build.cmd", "use pnpm, never npm");
    e.tags = "build,cli";
    write_fact(&root, &e).unwrap();

    let got = read_fact(&root, ws, "build.cmd")
        .unwrap()
        .expect("刚写的读得到");
    assert_eq!(got.tags, "build,cli", "tags 原样读回：{got:?}");
    assert_eq!(got.key, "build.cmd", "身份取 frontmatter name");
    assert_eq!(got.value, "use pnpm, never npm");
    assert_eq!(got.source, "explicit");
    assert_eq!(got.updated_at, e.updated_at);
    // hook 不许漂：tags 非空时 description 仍是 tags（与 render 侧同一条规则）
    let text = std::fs::read_to_string(&got.path).unwrap();
    assert!(
        text.contains("description: \"build,cli\""),
        "hook 仍取 tags：{text}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 旧文件（tags 一等字段之前落的）必须**照读**：缺那行 = 空串，不许 Err、不许丢
/// 正文、不许顺手回填。这文件形状是照 `render()` 去掉 `tags:` 那一行抄的（生产
/// 构造代码是唯一依据，不凭想象补字段）。
#[test]
fn a_pre_tags_file_reads_back_empty_tags_not_an_error() {
    let root = std::env::temp_dir().join(format!("memlayout-old-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let ws = r"F:\proj\Old";
    let dir = root
        .join("projects")
        .join(project_dir_name(ws))
        .join("facts");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("legacy.key.md"),
        "---\nname: \"legacy.key\"\ndescription: \"first line\"\nmetadata:\n  \
         type: fact\n  source: \"organized\"\n  workspace: \"F:\\\\proj\\\\Old\"\n  updated: 1700000000\n---\n\nfirst line\nsecond line\n",
    )
    .unwrap();

    let got = read_fact(&root, ws, "legacy.key")
        .unwrap()
        .expect("旧文件照样读得到");
    assert_eq!(got.tags, "", "缺 tags 行 = 空串，不报错");
    assert_eq!(got.source, "organized", "source 读回");
    assert_eq!(got.updated_at, 1700000000);
    assert_eq!(got.value, "first line\nsecond line", "正文一行不许丢");

    // 同一个夹具里再加一种：**人自己新建的裸文本文件**（没有 frontmatter）——身份
    // 回落文件名主干、全文就是正文。少了这一档，测试 2 只测到"缺行给空串"那种兜底，
    // 撤掉 parse_fact 的 tags 分支也不会红 = 零证明力。
    std::fs::write(dir.join("notes.md"), "hand written body\n").unwrap();
    let bare = read_fact(&root, ws, "notes")
        .unwrap()
        .expect("人新建的裸文件照样读得到");
    assert_eq!(bare.key, "notes", "无 frontmatter ⇒ 身份回落文件名主干");
    assert_eq!(bare.value, "hand written body", "全文即正文");
    assert_eq!(bare.tags, "");
    let _ = std::fs::remove_dir_all(&root);
}

/// 扫全根：排序口径必须与现役 SQL 逐字同形（`ORDER BY updated_at DESC, key`），
/// 且**"读不到目录"与"读不动文件"要能分开**——召回侧那条「库不可用不许冒充库是
/// 空的」的纪律搬到文件制之后，就靠 `unreadable` 这个计数活着。
#[test]
fn iter_facts_orders_like_the_sql_and_counts_unreadable_separately() {
    let root = std::env::temp_dir().join(format!("memlayout-iter-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let ws = r"F:\proj\Iter";
    for (key, at) in [("a.old", 1_000i64), ("b.new", 3_000), ("c.mid", 2_000)] {
        // 结构体更新语法（`..entry(…)`）而不是逐字段重列：FactEntry 在 S1 刀③ 会加
        // `ty` 字段，逐字段写死的夹具每加一个字段就要改一遍，而漏掉的那遍是**编译错误**
        // 而不是"测到的东西变少"——用 FRU 之后未列的字段自动沿默认。
        let e = FactEntry {
            updated_at: at,
            ..entry(ws, key, key)
        };
        write_fact(&root, &e).unwrap();
    }
    // 全局层一条（跨层必须一起扫到）
    let g = FactEntry {
        updated_at: 2_500,
        ..entry("", "g.global", "g")
    };
    write_fact(&root, &g).unwrap();
    // 一个"文件"其实是目录 ⇒ read_to_string 失败 ⇒ 只能进 unreadable，不许被当成没有
    std::fs::create_dir_all(
        root.join("projects")
            .join(project_dir_name(ws))
            .join("facts")
            .join("broken.md"),
    )
    .unwrap();

    let scan = iter_facts(&root);
    let keys: Vec<&str> = scan.facts.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(
        keys,
        vec!["b.new", "g.global", "c.mid", "a.old"],
        "updated_at 降序、同值再按 key 升序：{keys:?}"
    );
    assert_eq!(scan.unreadable, 1, "读不动的那条必须计数，不许静默吞掉");
    assert_eq!(
        scan.facts.iter().filter(|f| f.workspace.is_empty()).count(),
        1,
        "全局层 workspace = ''"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 层级故障进账（S1 刀②c-1 坐实）。夹具 = **在同一位置放一个普通文件冒充目录**：
/// `read_dir` 两侧都失败（POSIX `ENOTDIR` / Windows `ERROR_DIRECTORY`），且都不是
/// `NotFound` ⇒ 与"层不存在"天然可分。刀②a 登记这一档时说"没有跨平台夹具"（当时想的
/// 是 `chmod 000` / ACL），这条更普通的形态把那个理由推翻了，所以补上并留红证。
///
/// 同一份夹具里先跑两遍**负对照**（根都不在场、以及只有 `global/` 在场）：少了它们，
/// 实现漂成"任何 `read_dir` 失败都算故障"也不会红——而那种漂法会把每个新用户的空库
/// 读成"有东西扫不动"，正是这一族判据要防的另一半。
#[test]
fn a_blocked_layer_dir_is_counted_and_not_read_as_nothing() {
    let root = tmp_root("blocked-layer");
    let absent = tmp_root("blocked-absent");

    assert_eq!(
        iter_facts(&absent).unreadable,
        0,
        "负对照①：整个 md 根还不存在 = 从没记过，不是故障"
    );

    write_fact(&root, &entry("", "g.one", "全局那条")).unwrap();
    assert_eq!(
        iter_facts(&root).unreadable,
        0,
        "负对照②：`projects/` 缺席同样不是故障（只有全局层的新库是常态）"
    );
    assert_eq!(iter_facts(&root).facts.len(), 1);

    std::fs::write(projects_dir(&root), "not a directory").unwrap();
    let blocked = iter_facts(&root);
    assert_eq!(blocked.facts.len(), 1, "读得到的那一层照常交付");
    assert_eq!(
        blocked.unreadable, 1,
        "整层扫不动必须进账：真源 = 文件之后，不进账就与「这个项目从没记过」逐字同形"
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&absent);
}

/// 层级故障的第二档：项目文件夹在场，但它 `facts/` 的位置上坐着一个文件。
/// 这一档与上一条**不同源**——它由 `is_dir()` 那个分支管，把上面那条计数撤掉
/// 这里照样绿（反之亦然），所以必须单独一条。
#[test]
fn a_facts_slot_held_by_a_file_counts_that_project_as_blocked() {
    let root = tmp_root("blocked-facts");
    let alpha = r"F:\proj\Alpha";
    let beta = r"F:\proj\Beta";
    write_fact(&root, &entry(alpha, "a.one", "甲项目那条")).unwrap();

    let beta_dir = project_dir(&root, beta).unwrap();
    std::fs::create_dir_all(&beta_dir).unwrap();
    std::fs::write(beta_dir.join("facts"), "not a directory").unwrap();

    let scan = iter_facts(&root);
    assert_eq!(scan.facts.len(), 1, "另一个项目照常读到");
    assert_eq!(
        scan.unreadable, 1,
        "乙项目的 facts 位上是文件 ⇒ 它一条也扫不出来，得进账而不是隐身"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 清空真源（刀②c-1 的新清点名）。三条性质一条用例里跑完，因为它们同一个根：
/// ① 每条事实都算数，**包括人手写的那种**——它没 frontmatter，读回的 workspace 是
/// 目录名兜底，清空若按"逐行反推键位再 `delete_fact`"实现就会指到别的目录，
/// 结果是删不掉也不报错（`LayerDir::hint` 的警告就是这条红证逼出来的）；
/// ② 索引留在原地并被刷成空壳（"这层现在没东西"要自描述，不是文件失踪）；
/// ③ 清完之后 `unreadable` 仍是 0（不许留下一堆扫不动的残骸冒充"已清空"）。
#[test]
fn clear_facts_removes_every_fact_including_one_a_human_wrote_by_hand() {
    let root = tmp_root("clear");
    let ws = r"F:\proj\Clear";
    write_fact(&root, &entry(ws, "p.one", "项目那条")).unwrap();
    write_fact(&root, &entry("", "g.one", "全局那条")).unwrap();
    let facts = project_dir(&root, ws).unwrap().join("facts");
    std::fs::write(facts.join("hand.md"), "hand written body\n").unwrap();

    let n = clear_facts(&root).unwrap();
    assert_eq!(n, 3, "三条全部数进去（含人手建那条）：{n}");
    assert!(!facts.join("hand.md").exists(), "人手写那条也得删掉");
    assert!(!root.join(GLOBAL_DIR).join("g.one.md").exists());

    let index = std::fs::read_to_string(root.join(GLOBAL_DIR).join("MEMORY.md")).unwrap();
    assert!(
        index.starts_with("# 记忆索引"),
        "索引文件还在，只是空了：{index}"
    );
    assert!(!index.contains("g.one"), "索引跟着刷空：{index}");
    let after = iter_facts(&root);
    assert!(after.facts.is_empty());
    assert_eq!(after.unreadable, 0, "清空之后不许留下扫不动的残骸");
    let _ = std::fs::remove_dir_all(&root);
}

/// 有一层扫不动 ⇒ 清空**中止且不改动任何文件**。这不是过度谨慎：清空是要向用户
/// 报数的动作（"已清空 n 条"），半删状态下一句谎话比一次失败更难被发现。
#[test]
fn clear_facts_refuses_when_a_layer_cannot_be_scanned() {
    let root = tmp_root("clear-blocked");
    write_fact(&root, &entry("", "g.one", "全局那条")).unwrap();
    std::fs::write(projects_dir(&root), "not a directory").unwrap();

    let e = clear_facts(&root).unwrap_err();
    assert!(e.contains("扫不动"), "报错要点名是层级故障：{e}");
    assert_eq!(
        iter_facts(&root).facts.len(),
        1,
        "中止必须真的中止：原有事实一条也不许动"
    );
    let _ = std::fs::remove_dir_all(&root);
}
