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
        let e = FactEntry {
            workspace: ws,
            key,
            value: key,
            tags: "",
            source: "explicit",
            updated_at: at,
        };
        write_fact(&root, &e).unwrap();
    }
    // 全局层一条（跨层必须一起扫到）
    let g = FactEntry {
        workspace: "",
        key: "g.global",
        value: "g",
        tags: "",
        source: "explicit",
        updated_at: 2_500,
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
