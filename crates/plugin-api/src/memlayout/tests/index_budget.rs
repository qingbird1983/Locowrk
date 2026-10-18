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
