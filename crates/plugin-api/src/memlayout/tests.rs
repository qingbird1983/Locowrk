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

// 只在 Windows 跑：`F:\Warehouse\AIwork\Locowrk` 这类反斜杠串在 POSIX 上是**一个合法文件名**
// 而不是一串目录，"取末段"这件事在两个平台上语义不同（Linux 侧算式不剥 `\` 不是缺陷）。
#[cfg(windows)]
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

/// 队列 Q15：「这个项目的人类层家目录在哪」全仓只许一处算法，且它对未归属
/// **拒答**。断言里的期望路径全部手抄（不叫 `project_dir` 自己给自己当参照），
/// 变异 M1（把 `''` 那半撤掉）打的正是这里。
#[test]
fn project_dir_rejects_unowned_and_is_the_single_formula() {
    let root = std::path::Path::new(r"X:\data\memory");
    assert!(
        project_dir(root, "").is_err(),
        "未归属不是项目：家目录必须拒答，而不是给一个伪项目目录（那是级联删除能吃下全局键位的入口）"
    );
    let ws = r"F:\Warehouse\AIwork\a-proj";
    let dir = project_dir(root, ws).expect("项目支有落点");
    assert_eq!(
        dir,
        root.join("projects").join(project_dir_name(ws)),
        "绝对落点 = projects/<可读名-短哈希>"
    );
    // 相对支（写事实用的那个）与绝对支必须同出一处：拼上 root 后逐段相等
    assert_eq!(
        root.join(fact_rel_path(ws, "k1")),
        dir.join("facts").join("k1.md"),
        "事实文件 = 家目录/facts/<stem>.md，两份投影同源"
    );
    assert_eq!(
        unaffiliated_dir(root),
        root.join("projects").join("_unaffiliated"),
        "未归属的 tree.md 之家固定 _unaffiliated（走 PROJECTS_DIR 一处字面量）"
    );
    assert_ne!(
        unaffiliated_dir(root),
        dir,
        "未归属不得被并成伪项目目录（并了 = 改磁盘形状）"
    );
}

/// 恶意 / 畸形 workspace 串只能落进 root 之内：目录名是「安全化 + fnv 短哈希」
/// 的产物，`..` 与分隔符都进不了那一个名字。新命令收的是 workspace 而不是路径，
/// 所以这条性质就是它的防线本身（比 `reveal_in_explorer` 的 fsRoots 白名单更窄）。
// 只在 Windows 跑：这批恶意串（`C:\Windows\..`、`a\b\..\c`）按 Windows 分隔符判"有没有穿越材料"。
// POSIX 上反斜杠是普通字符，`a\b\..\c` 就是**一个目录名**、不构成穿越 ⇒ 安全性质仍在，
// 只是这条判读法不适用（Linux 上该用 `/` 形态的串重做一批用例 = 真要支持 Linux 时的活，见 T18）。
#[cfg(windows)]
#[test]
fn project_dir_keeps_any_workspace_string_under_the_root() {
    let root = std::path::Path::new(r"X:\data\memory");
    for evil in [
        "../../etc",
        r"C:\Windows\..",
        "..",
        r"..\..\windows",
        " ",
        "F:/a/b",
        "a\\b\\..\\c",
    ] {
        let dir = project_dir(root, evil).unwrap_or_else(|e| panic!("{evil:?} 应有家目录: {e}"));
        assert!(dir.starts_with(root), "{evil:?} 的落点跑出 root: {dir:?}");
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            !name.contains('/') && !name.contains('\\') && !name.contains(".."),
            "{evil:?} 的目录名含穿越材料: {name}"
        );
    }
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

// ---- F2 同步机械件：标记 / 解析 / 冲突备份 ----

fn tmp_root(tag: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("memlayout-f2-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    root
}

#[test]
fn write_fact_records_sync_marker_and_delete_removes_it() {
    let root = tmp_root("marker");
    write_fact(&root, &entry("", "m.k", "v")).unwrap();
    let rel = fact_rel_path("", "m.k");
    let m = read_marker(&root, &rel).expect("标记随写落盘");
    assert_eq!(m, mtime_ms(&root.join(&rel)).unwrap(), "标记 = 落笔 mtime");
    delete_fact(&root, "", "m.k").unwrap();
    assert!(read_marker(&root, &rel).is_none(), "删除随行删标记");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn parse_fact_roundtrips_frontmatter_and_falls_back_to_body() {
    let text = entry("ws", "k\"1", "line1\nline2").render();
    let p = parse_fact(&text);
    assert_eq!(p.name.as_deref(), Some("k\"1"), "引号反转义");
    assert_eq!(p.workspace.as_deref(), Some("ws"));
    assert_eq!(p.source.as_deref(), Some("explicit"));
    assert_eq!(p.updated_at, Some(1728100000));
    assert_eq!(p.body, "line1\nline2");
    let bare = parse_fact("# 标题\n\n正文");
    assert_eq!(bare.name, None, "无 frontmatter = 无名");
    assert_eq!(bare.body, "# 标题\n\n正文", "全文即正文");
}

#[test]
fn human_edit_before_model_write_is_backed_up_not_lost() {
    let root = tmp_root("stash");
    write_fact(&root, &entry("", "k", "model v1")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(25));
    let rel = fact_rel_path("", "k");
    // 人直接改文件（正文换掉，frontmatter 保留旧 updated）
    std::fs::write(
        root.join(&rel),
        "---\nname: \"k\"\ndescription: \"d\"\nmetadata:\n  type: fact\n  source: \"explicit\"\n  workspace: \"\"\n  updated: 1728100000\n---\n\nhuman edit\n",
    )
    .unwrap();
    // 模型随后同键写入 → 冲突备份，主键 = 时间戳新者（模型）
    let p = write_fact(&root, &entry("", "k", "model v2")).unwrap();
    let main = std::fs::read_to_string(&p).unwrap();
    assert!(main.contains("model v2"), "主键文件 = 新者");
    let backups: Vec<String> = std::fs::read_dir(root.join("global"))
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().ends_with(".md"))
        .filter(|e| e.file_name().to_string_lossy() != "k.md")
        .filter(|e| e.file_name().to_string_lossy() != "MEMORY.md")
        .map(|e| std::fs::read_to_string(e.path()).unwrap())
        .collect();
    assert_eq!(backups.len(), 1, "恰好一份人工备份");
    let parsed = parse_fact(&backups[0]);
    assert!(
        parsed
            .name
            .as_deref()
            .unwrap_or_default()
            .starts_with("k · 人工编辑备份 "),
        "备份键自描述：{:?}",
        parsed.name
    );
    assert_eq!(parsed.body.trim_end(), "human edit", "人的版本保全");
    // 同内容重复写不再产生备份
    write_fact(&root, &entry("", "k", "model v2")).unwrap();
    let backups2 = std::fs::read_dir(root.join("global"))
        .unwrap()
        .flatten()
        .filter(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            n.ends_with(".md") && n != "k.md" && n != "MEMORY.md"
        })
        .count();
    assert_eq!(backups2, 1, "内容相同 = 无冲突");
    let _ = std::fs::remove_dir_all(&root);
}
