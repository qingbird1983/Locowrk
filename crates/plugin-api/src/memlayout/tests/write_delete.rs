//! 写路径的同步机械件（自 `tests.rs` 搬入，用例本体逐字未改）：写 → 索引刷新 →
//! 删除往返、标记随行、**人工编辑先于模型写入时保住双方**，以及格式升级不得被
//! 误判成人工编辑。最后那条是 S1 刀②a 新增的：不守住它，每升一次渲染格式就给
//! 每个键位造一条假备份，人打开文件夹看到的是满屏自己没写过的文件。

use super::*;

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

/// 格式升级不许被误判成"人工编辑"而造出一条假备份。判据形状在这里说破：
/// `stash_human_edit` 只在 **mtime 变了且内容 ≠ 本次渲染** 时备份，所以给旧格式
/// 文件补上 `tags:` 行再写第二次 = 时间戳未动 = 无备份；真改了内容才备份。
#[test]
fn format_upgrade_does_not_fabricate_a_human_edit_backup() {
    let root = std::env::temp_dir().join(format!("memlayout-upg-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let ws = r"F:\proj\Upg";
    let dir = root
        .join("projects")
        .join(project_dir_name(ws))
        .join("facts");
    std::fs::create_dir_all(&dir).unwrap();
    // 第一份：手抄"没有 tags 行"的旧渲染，并打上 mtime 基线（= 生产 write_fact 做的事）
    let rel = fact_rel_path(ws, "k");
    let path = root.join(&rel);
    std::fs::write(
        &path,
        "---\nname: \"k\"\ndescription: \"v1\"\nmetadata:\n  type: fact\n  \
         source: \"explicit\"\n  workspace: \"F:\\\\proj\\\\Upg\"\n  updated: 1700000000\n---\n\nv1\n",
    )
    .unwrap();
    let ms = mtime_ms(&path).unwrap();
    write_marker(&root, &rel, ms).unwrap();

    // 第二次：格式已升级（多一行 tags），但**没动过 mtime** ⇒ 不该有备份
    // （`..entry(…)` = 结构体更新语法，理由见 `read_back.rs` 里那句注释）
    let new_form = FactEntry {
        updated_at: 1700000000,
        ..entry(ws, "k", "v1")
    };
    assert!(
        new_form.render().contains("tags: \"\""),
        "前提：本次渲染确实多了 tags 这一行（否则这测试什么都没测）"
    );
    write_fact(&root, &new_form).unwrap();
    // 备份件的识别口径**照既有那条备份测试**（`human_edit_before_model_write_is_backed_up_not_lost`）：
    // 键位里中文与空格会被 key_file_stem 换成 `_`，所以不能按"人工编辑备份"字样数——
    // 那样数永远 0，前半段断言就成了零证明力的空转。
    let backups = || {
        std::fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .filter(|e| {
                let n = e.file_name().to_string_lossy().into_owned();
                n.ends_with(".md") && n != "k.md" && n != "MEMORY.md"
            })
            .count()
    };
    assert_eq!(backups(), 0, "格式升级不该造出假的人工编辑备份");

    // 真编辑：内容改了且 mtime 变新 ⇒ 必须备份（这条判据还在承重）
    std::fs::write(
        &path,
        "---\nname: \"k\"\ndescription: \"v1\"\ntags: \"\"\nmetadata:\n  type: fact\n  \
         source: \"explicit\"\n  workspace: \"F:\\\\proj\\\\Upg\"\n  updated: 1700000000\n---\n\nhuman edit\n",
    )
    .unwrap();
    let newer = ms + 60_000;
    filetime_set(&path, newer);
    write_fact(&root, &new_form).unwrap();
    assert_eq!(backups(), 1, "人真改过就必须保住双方：{} 件", backups());
    let _ = std::fs::remove_dir_all(&root);
}

/// 把 mtime 推到指定毫秒（不必等真实时钟）。set_modified 要 SystemTime，
/// 这里用 UNIX_EPOCH + 毫秒直接算——测试里唯一一处"人为造时间"的地方。
fn filetime_set(path: &std::path::Path, millis: i64) {
    let t = std::time::UNIX_EPOCH + std::time::Duration::from_millis(millis as u64);
    std::fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

/// 「这一层读不动」与「没有这条记忆」必须是**两个答案**（S1 刀②c-1）。
///
/// 逼出这条的是一个跨平台差异，不是猜测：Windows 的 `remove_file` 对"路径里有一段
/// 不是目录"报 `ERROR_PATH_NOT_FOUND`，而它与"文件不存在"是**同一个错误码**；
/// POSIX 才分成 `ENOTDIR` 与 `ENOENT`。不特判就会得到一条"Linux 报错、Windows 说
/// 没这条记忆"的假性质——正好是本刀要消灭的那种"把读不动冒充成没有"。
/// 对照那半条同样承重：层不存在时仍必须是 `Ok(false)`（幂等删除），否则实现漂成
/// "什么都报错"也不会红。
#[test]
fn deleting_into_a_blocked_layer_errors_instead_of_claiming_absence() {
    let root = tmp_root("delete-blocked");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join(GLOBAL_DIR), "not a directory").unwrap();

    let e = delete_fact(&root, "", "any.key").expect_err("层不是目录时要报错，不许谎称没这条");
    assert!(e.contains("不是目录"), "报错得说是哪种故障：{e}");

    let absent = tmp_root("delete-absent");
    assert!(
        !delete_fact(&absent, "", "any.key").unwrap(),
        "对照：层根本不在场 = 没有这条（幂等），不是故障"
    );
    let _ = std::fs::remove_dir_all(&root);
    let _ = std::fs::remove_dir_all(&absent);
}
