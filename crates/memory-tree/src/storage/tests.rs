//! storage.rs 的单测：投影目录命名（scope_dir）、归一后同目录、存取往返与原子写。
//!
//! 本文件是**宿主 .rs + 同名子目录**形态（v2.197 从 storage.rs 的内联 `mod tests`
//! 整块摘出 = `check_length` 报本文件超限 427 行的直接后果）。这种形态**不在第五道
//! 形状闸的管辖内**（它只收带 `mod.rs` 的目录，历批 Q8c 第⑤条实测过这个空白档），
//! 所以摘出后必须同时满足两条：宿主文件里留着 `#[cfg(test)] mod tests;` 声明，且
//! 本文件非空。两条都由 `tmp/q97-shape.mjs` 的 E5 钉住（声明被摘掉时 cargo 与闸都
//! 不红、静默少跑用例 = 历批那个洞，这里不重演）。

use super::*;
use crate::tree::RunSummary;

fn sample(ws: &str, sid: &str) -> MemoryTree {
    let mut t = MemoryTree::new(ws, sid);
    t.insert_placeholder(0, 4, 1).unwrap();
    t
}

#[test]
fn scope_dir_is_stable_readable_and_collision_safe() {
    assert_eq!(scope_dir(""), "_root");
    let a = scope_dir(r"F:\Warehouse\AIwork\Locowrk");
    let b = scope_dir(r"F:\Warehouse\AIwork\Locowrk");
    assert_eq!(a, b, "同一工作区两次编码必须一致");
    assert!(a.starts_with("F__Warehouse"));
    assert_ne!(a, scope_dir(r"F:\Warehouse\AIwork\Other"));
}

/// Q7c（v2.197）：目录名吃的是**归一后的键位**，不是旁车原样串。宿主
/// `workspace_view` 在源头归一之后，同一个项目的几种写法在这把尺上必须合成**一个**
/// 目录——本批实测磁盘上同项目长出两个目录（`F__Warehouse_TITY-94676677` 与
/// `tity-9242b25e`），根因就是喂给这里的串没归一。路径形态取 `/ws/proj` 是刻意的：
/// 大小写折叠只有 Windows 有，把它写进期望会得到一条 POSIX 红的平台假判据。
#[test]
fn scope_dir_of_normalized_variants_is_one_directory() {
    let raw = "/ws/proj";
    let key = plugin_api::wskey::normalize(raw);
    // 跨平台都成立的一档：尾分隔符漂移
    assert_eq!(
        scope_dir(&plugin_api::wskey::normalize(&format!("{raw}/"))),
        scope_dir(&key),
        "尾分隔符漂移经归一不许造出第二个投影目录"
    );
    // ⚠ 这一条第一版写的是 `normalize(raw.replace('\\', "/"))` 与 `raw` 比 —— raw 本就是
    //   斜杠形态，替换是 no-op ⇒ 两边同一个串，断言**恒真**（第 66 条「恒真比恒假更险」
    //   在本批写完当场自查抓到）。正解 = 拿真漂移的形态喂进去，并且**按平台写期望**：
    //   反斜杠在 POSIX 是合法文件名字符，那里合并不对。
    #[cfg(windows)]
    assert_eq!(
        scope_dir(&plugin_api::wskey::normalize(r"\ws\proj")),
        scope_dir(&key),
        "Windows 上斜杠方向漂移经归一必须合到同一个目录"
    );
    #[cfg(not(windows))]
    assert_ne!(
        scope_dir(&plugin_api::wskey::normalize(r"\ws\proj")),
        scope_dir(&key),
        "POSIX 上二者本就是两个目录名，那里合并才是错"
    );
    // 不归一时它们就是两个目录——这正是本批磁盘上看到的形状
    assert_ne!(
        scope_dir(raw),
        scope_dir(&format!("{raw}/")),
        "未归一的尾分隔符漂移会开出第二个目录（缺陷的正面描述）"
    );
}

#[test]
fn save_load_roundtrip_and_atomic_tmp_leftover() {
    let dir = std::env::temp_dir().join(format!("mt-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = TreeStore::new(&dir);
    let t = sample("ws-a", "sess-1");
    store.save(&t).expect("保存成功");
    let loaded = store.load("ws-a", "sess-1").expect("可读回");
    assert_eq!(loaded.pending.as_ref().unwrap().from, 0);
    // 同一工作区再次保存不因 tmp 残留失败
    store.save(&sample("ws-a", "sess-1")).expect("二次保存");
    assert!(store.delete("ws-a", "sess-1").unwrap());
    assert!(store.load("ws-a", "sess-1").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn dotted_session_id_round_trips() {
    // 回归锁（审计 M-23）：本侧曾拒 `.`（而 session-store 放行）——含点
    // 的会话 id「日志有、树投影存不进」，只以 Err 字符串出现 = 静默
    // 失效。规则单点化（plugin_api::naming）后必须存得进、读得出。
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let dir = std::env::temp_dir().join(format!("mt-test-dotted-{nanos}"));
    let _ = std::fs::remove_dir_all(&dir);
    let store = TreeStore::new(&dir);
    let id = "a.b_c-1";
    store.save(&sample("ws-a", id)).expect("带点 id 必须存得进");
    assert!(store.load("ws-a", id).is_some(), "读得出");
    assert!(
        store
            .scan_trees()
            .iter()
            .any(|s| s.session_id == id && s.workspace == "ws-a"),
        "清单一并可见（投影可被发现才算真活）"
    );
    assert!(
        store.load("ws-a", ".").is_none(),
        "整串全点必须拒（读面回落 None = 按不存在处理）"
    );
    assert!(
        store.save(&sample("ws-a", "../escape")).is_err(),
        "路径分隔必须拒"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bad_session_id_rejected_and_bad_json_is_none() {
    let dir = std::env::temp_dir().join(format!("mt-test-bad-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = TreeStore::new(&dir);
    assert!(store.load("ws", "../evil").is_none());
    assert!(store.save(&sample("ws", "a/b")).is_err());
    // 坏 JSON = 无投影
    let scope = store.root.join(scope_dir("ws-x"));
    std::fs::create_dir_all(&scope).unwrap();
    std::fs::write(scope.join("bad.json"), b"{ not json").unwrap();
    assert!(store.load("ws-x", "bad").is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scan_pending_finds_only_pending_trees() {
    let dir = std::env::temp_dir().join(format!("mt-test-scan-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = TreeStore::new(&dir);
    store.save(&sample("ws-a", "sess-1")).unwrap();
    let mut clean = MemoryTree::new("ws-a", "sess-2");
    clean.chaptered_until = 3;
    store.save(&clean).unwrap();
    let hits = store.scan_pending();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].session_id, "sess-1");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scan_workspace_lists_single_scope_with_mtime() {
    let dir = std::env::temp_dir().join(format!("mt-test-ws-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = TreeStore::new(&dir);
    store.save(&sample("ws-a", "sess-2")).unwrap();
    store.save(&sample("ws-a", "sess-1")).unwrap();
    store.save(&sample("ws-b", "sess-3")).unwrap();
    let rows = store.scan_workspace("ws-a");
    assert_eq!(rows.len(), 2, "只取本工作区");
    assert_eq!(rows[0].1, "sess-1", "会话 id 稳定序");
    assert!(rows.iter().all(|r| r.2 > 0), "mtime 在场");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scan_trees_lists_all_sorted_by_recency() {
    let dir = std::env::temp_dir().join(format!("mt-test-list-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = TreeStore::new(&dir);
    store.save(&sample("ws-a", "sess-1")).unwrap();
    let mut clean = MemoryTree::new("ws-b", "sess-2");
    clean.chaptered_until = 7;
    store.save(&clean).unwrap();
    let list = store.scan_trees();
    assert_eq!(list.len(), 2, "有在途与无在途的树都进清单");
    assert!(list[0].updated_ms >= list[1].updated_ms, "按更新时间降序");
    let pending = list.iter().find(|s| s.session_id == "sess-1").unwrap();
    let clean = list.iter().find(|s| s.session_id == "sess-2").unwrap();
    assert_eq!(pending.chapter_count, 1, "占位章节计入章节数");
    assert!(pending.pending);
    assert_eq!(clean.chaptered_until, 7);
    assert!(!clean.pending);
    // 坏投影文件跳过
    let scope = store.root.join(scope_dir("ws-a"));
    std::fs::write(scope.join("bad.json"), b"{ not json").unwrap();
    assert_eq!(store.scan_trees().len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// 队列 Q14 第三刀：清单侧**一次**给出「N 总结」。看板左栏那一行写
/// `N 总结 · N 章节`（MB §3.1），如果清单不给这个数，前端只能对每个会话再发一次
/// `load` 才凑得齐——N 次读盘换一个数，比多带一个字段贵得多。
///
/// 两条口径在这里钉死：①计数取的是投影里的**总结条数**；②它与章节数是**两把尺**
/// ——一轮总结可能一个章节都没落成，反向也成立（有章节没总结），互相吞掉就报错了。
#[test]
fn scan_trees_reports_summary_count_independently_of_chapters() {
    let dir = std::env::temp_dir().join(format!("mt-test-sumcount-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = TreeStore::new(&dir);
    let mut t = MemoryTree::new("ws-a", "sess-1");
    t.insert_placeholder(0, 4, 1).unwrap();
    t.summaries.push(RunSummary {
        from: 0,
        to: 4,
        text: "第一轮".into(),
        created_at: 1,
    });
    t.summaries.push(RunSummary {
        from: 5,
        to: 9,
        text: "第二轮".into(),
        created_at: 2,
    });
    store.save(&t).unwrap();
    // 只有总结、一个章节都没有的树：两把尺必须各报各的
    let mut only_summaries = MemoryTree::new("ws-a", "sess-2");
    only_summaries.chaptered_until = 3;
    only_summaries.summaries.push(RunSummary {
        from: 0,
        to: 3,
        text: "降级那一轮".into(),
        created_at: 3,
    });
    store.save(&only_summaries).unwrap();

    let list = store.scan_trees();
    let with_both = list.iter().find(|s| s.session_id == "sess-1").unwrap();
    assert_eq!(with_both.summary_count, 2, "总结条数按投影里的条数报");
    assert_eq!(
        with_both.chapter_count, 1,
        "占位章节也计入章节数（与既有口径一致）"
    );
    let bare = list.iter().find(|s| s.session_id == "sess-2").unwrap();
    assert_eq!(bare.summary_count, 1);
    assert_eq!(bare.chapter_count, 0, "没有章节不把总结吞掉");
    let _ = std::fs::remove_dir_all(&dir);
}
