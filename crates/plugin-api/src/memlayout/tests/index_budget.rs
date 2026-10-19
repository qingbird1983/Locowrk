//! 索引写侧预算（自 `tests.rs` 搬入，用例本体逐字未改；S1 刀① 立）：
//! 硬钳落在**来盘侧**，且两条预算谁先撞线都要自描述。

use super::*;

/// 建 N 个事实文件（正文一律走生产渲染 `render()`，不手写 md——手写夹具测不到
/// `refresh_index` 读 frontmatter 那条路），然后只刷一次索引。
fn seed_facts(root: &std::path::Path, ws: &str, n: usize, key_pad: usize) -> std::path::PathBuf {
    let dir = root
        .join("projects")
        .join(project_dir_name(ws))
        .join("facts");
    std::fs::create_dir_all(&dir).unwrap();
    for i in 0..n {
        let key = format!("k.{i:0width$}", width = key_pad);
        std::fs::write(dir.join(format!("{key}.md")), entry(ws, &key, "v").render()).unwrap();
    }
    dir
}

fn index_text(root: &std::path::Path, ws: &str) -> String {
    std::fs::read_to_string(
        root.join("projects")
            .join(project_dir_name(ws))
            .join("MEMORY.md"),
    )
    .unwrap()
}

/// 写侧硬顶（设计 §2.1 + 决策人裁定④）：**钳落到来盘侧**，人打开 `MEMORY.md`
/// 看到的就该是模型看到的那份。撞顶必须自描述，静默少列等于告诉模型"就这些"。
#[test]
fn refresh_index_clamps_at_the_shared_budget() {
    let root = std::env::temp_dir().join(format!("memlayout-clamp-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let ws = r"F:\proj\Clamp";
    seed_facts(&root, ws, MAX_INDEX_LINES + 50, 4);
    refresh_index(&root, ws).unwrap();
    let index = index_text(&root, ws);
    let pointers = index.lines().filter(|l| l.starts_with("- [")).count();
    assert_eq!(
        pointers, MAX_INDEX_LINES,
        "落盘指针行恰好钳在 {MAX_INDEX_LINES}，实际 {pointers}"
    );
    assert!(
        index.contains(INDEX_TRUNCATED_NOTE),
        "超限必须写截断行：{index}"
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// 边界不误伤：刚好到顶 = 一条不丢、也不写截断行（宿主注入侧同款判据，
/// 两头口径必须一致，否则"文件里有但注入说不完整"又是一处各说各话）。
#[test]
fn refresh_index_at_exact_budget_is_not_truncated() {
    let root = std::env::temp_dir().join(format!("memlayout-edge-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let ws = r"F:\proj\Edge";
    seed_facts(&root, ws, MAX_INDEX_LINES, 4);
    refresh_index(&root, ws).unwrap();
    let index = index_text(&root, ws);
    assert_eq!(
        index.lines().filter(|l| l.starts_with("- [")).count(),
        MAX_INDEX_LINES,
        "刚好到顶不许少一行"
    );
    assert!(!index.contains(INDEX_TRUNCATED_NOTE), "刚好到顶不算截断");
    let _ = std::fs::remove_dir_all(&root);
}

/// 两条预算谁先说话：长 key 的索引行很宽，**字符预算应先撞线**（行数还远不到
/// 200）。这条是防"只钳行数"那种半截实现——真库里 key 合法上限是 512 字节，
/// 宽 key 是真实落盘形态，不是臆造夹具。
#[test]
fn refresh_index_char_budget_bites_before_line_budget() {
    let root = std::env::temp_dir().join(format!("memlayout-wide-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let ws = r"F:\proj\Wide";
    seed_facts(&root, ws, MAX_INDEX_LINES, 200);
    refresh_index(&root, ws).unwrap();
    let index = index_text(&root, ws);
    let pointers = index.lines().filter(|l| l.starts_with("- [")).count();
    assert!(
        pointers < MAX_INDEX_LINES,
        "字符预算应先撞线（少于 {MAX_INDEX_LINES} 条），实际 {pointers}"
    );
    assert!(index.contains(INDEX_TRUNCATED_NOTE), "先撞线同样要自描述");
    let _ = std::fs::remove_dir_all(&root);
}

/// `refresh_all_indexes` 管的是**盘上所有层**，不是"调用方知道的那几个 workspace"。
///
/// 与宿主 `memory_reconcile::tests` 里那条的分工：那条证"对账真的调了它"（红在改前——
/// 改前只刷"这一轮删过幽灵行的层"），这条证它自己交出的两个数：刷了几层、有几层没刷到。
/// 后者是 `layers()` 的既有纪律——扫不动的层**不进清单**，不单独交代就会被读成"这里没东西"
/// （刀④c 裁定③同一件事，那里是两个数，这里是其中一个）。
#[test]
fn refresh_all_indexes_rebuilds_every_layer_on_disk() {
    let root = std::env::temp_dir().join(format!("memlayout-all-layers-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let global = root.join(paths::GLOBAL_DIR);
    std::fs::create_dir_all(&global).unwrap();
    for key in ["alpha", "beta"] {
        std::fs::write(
            global.join(format!("{key}.md")),
            entry("", key, "v").render(),
        )
        .unwrap();
    }
    // 一个项目层：目录照算式建出来，而**权威原串此后不告诉任何调用方**——这正是人在文件
    // 管理器里手工丢出来的那一层的形状。按 `scopes` 逐层刷的那种写法在这里会漏掉它。
    let ws = r"F:\proj\Orphan";
    seed_facts(&root, ws, 1, 1);
    let (done, skipped) = refresh_all_indexes(&root);
    assert_eq!(skipped, 0, "两层都该刷得动，刷了 {done} 层");
    assert_eq!(done, 2, "全局层 + 那个没人声明过的项目层：{done}");
    let gtext = std::fs::read_to_string(global.join("MEMORY.md")).unwrap();
    assert!(
        gtext.contains("[alpha](alpha.md)") && gtext.contains("[beta](beta.md)"),
        "全局层索引要按盘上文件重建：{gtext}"
    );
    let ptext = index_text(&root, ws);
    assert!(ptext.contains("[k.0](k.0.md)"), "项目层同样要重建：{ptext}");
    let _ = std::fs::remove_dir_all(&root);
}

/// 扫不动的那一层必须从**第二返回值**交代（夹具 = `projects/broken/facts` 拿一个普通文件
/// 冒充目录，与 `read_back.rs` 那两条同形）。不交代的话"这一层读不动"和"这一层没记过东西"
/// 就是同一个答案——真源 = 文件之后没有第二个源能揭穿它。
#[test]
fn refresh_all_indexes_reports_layers_it_could_not_reach() {
    let root = std::env::temp_dir().join(format!("memlayout-skipped-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let broken = paths::projects_dir(&root).join("broken");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("facts"), "这不是目录").unwrap();
    let global = root.join(paths::GLOBAL_DIR);
    std::fs::create_dir_all(&global).unwrap();
    std::fs::write(global.join("a.md"), entry("", "a", "v").render()).unwrap();
    let (done, skipped) = refresh_all_indexes(&root);
    assert_eq!(done, 1, "只有全局层刷得动，实际 {done}");
    assert_eq!(
        skipped, 1,
        "那一层要被点名，不许演成『这里没东西』（全局层永远在场，所以它不该是 0）"
    );
    let _ = std::fs::remove_dir_all(&root);
}
