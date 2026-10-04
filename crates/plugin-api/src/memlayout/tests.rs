//! memlayout 单测：目录命名 / 文件名安全 / 渲染 / 写删索引往返。

use super::*;

fn entry<'a>(ws: &'a str, key: &'a str, value: &'a str) -> FactEntry<'a> {
    FactEntry {
        workspace: ws,
        key,
        value,
        tags: "",
        source: "explicit",
        updated_at: 1728100000,
    }
}

#[test]
fn project_dir_name_is_readable_and_collision_safe() {
    let a = project_dir_name(r"F:\Warehouse\AIwork\Locowrk");
    assert!(a.starts_with("Locowrk-"), "可读前缀取路径末段：{a}");
    assert_eq!(a.len(), "Locowrk-".len() + 8, "短哈希 = 8 hex");
    assert_eq!(
        a,
        project_dir_name(r"F:\Warehouse\AIwork\Locowrk"),
        "同路径恒等"
    );
    assert_ne!(
        project_dir_name(r"D:\code\Locowrk"),
        a,
        "同名不同路径不冲突（哈希区分）"
    );
    assert!(
        project_dir_name("").starts_with("project-"),
        "空路径兜底可读名"
    );
}

#[test]
fn key_file_stem_is_filename_safe_and_reserved_aware() {
    assert_eq!(key_file_stem("user.language"), "user.language", "点号保留");
    assert_eq!(key_file_stem(r"a/b\c:d"), "a_b_c_d", "路径字符换下划线");
    assert_eq!(key_file_stem("con"), "_con", "Windows 保留名前缀");
    assert_eq!(key_file_stem("com10.md"), "com10.md", "com 后非 1–9 不保留");
    assert_eq!(key_file_stem("..."), "_", "全点剥成占位");
}

#[test]
fn render_fact_has_frontmatter_and_description_fallback() {
    let with_tags = entry("ws", "k1", "line1\nline2").render();
    assert!(with_tags.contains("name: \"k1\""));
    assert!(with_tags.contains("metadata:"));
    assert!(with_tags.contains("  workspace: \"ws\""));
    assert!(with_tags.ends_with("line1\nline2\n"), "正文原样收尾");
    // description 落 tags；tags 空 = 正文首行
    assert!(!with_tags.contains("tags_in_desc"));
    let mut e = entry("ws", "k1", "first line\nsecond");
    assert!(
        e.render().contains("description: \"first line second\""),
        "hook 单行化"
    );
    e.tags = "pref";
    assert!(e.render().contains("description: \"pref\""));
}

#[test]
fn write_delete_roundtrip_updates_index() {
    let root = std::env::temp_dir().join(format!("memlayout-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let ws = r"F:\proj\Alpha";
    let p = write_fact(&root, &entry(ws, "db.choice", "postgres")).expect("写成功");
    assert!(
        p.ends_with("facts/db.choice.md"),
        "项目落 projects/<dir>/facts/：{}",
        p.display()
    );
    let g = write_fact(&root, &entry("", "pref.pnpm", "always pnpm")).expect("写成功");
    assert!(g.ends_with(r"global\pref.pnpm.md") || g.ends_with("global/pref.pnpm.md"));

    let index = std::fs::read_to_string(
        root.join("projects")
            .join(project_dir_name(ws))
            .join("MEMORY.md"),
    )
    .unwrap();
    assert!(
        index.contains("- [db.choice](db.choice.md) — postgres"),
        "索引行含正文首行 hook：{index}"
    );
    // 覆盖写后索引仍一行
    write_fact(&root, &entry(ws, "db.choice", "sqlite")).unwrap();
    let index = std::fs::read_to_string(
        root.join("projects")
            .join(project_dir_name(ws))
            .join("MEMORY.md"),
    )
    .unwrap();
    assert_eq!(
        index.lines().filter(|l| l.contains("db.choice")).count(),
        1,
        "覆盖写后索引仍一行"
    );
    // 删除：文件消失 + 索引同步；再删 = Ok(false)
    assert!(delete_fact(&root, ws, "db.choice").unwrap());
    assert!(
        !root
            .join("projects")
            .join(project_dir_name(ws))
            .join("facts/db.choice.md")
            .exists()
    );
    assert!(!delete_fact(&root, ws, "db.choice").unwrap(), "幂等删除");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn enabled_from_env_semantics() {
    assert!(enabled_from(None), "env 未设 = 开");
    assert!(enabled_from(Some("1")));
    assert!(!enabled_from(Some("0")), "仅 \"0\" = 关");
}
