//! memory 存储单测：CRUD / 过滤 / 上限校验 / 旧库迁移 / 项目键位隔离
//! （㉘）/ MemorySource 投影。全部走临时目录（LOCOWRK_DATA_DIR 无涉——
//! MemoryStore 显式持目录）。

use super::MemoryStore;
use kernel_agent::MemorySource;
use std::path::PathBuf;
use std::sync::Arc;

fn temp_dir(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "locowrk-memstore-{tag}-{nanos}-{}",
        std::process::id()
    ))
}

/// 测试解析器：`ws-session` → 项目根，其余未归属。
fn test_resolver() -> crate::workspace::WorkspaceResolver {
    Arc::new(|sid: &str| (sid == "ws-session").then(|| "F:/ws/a".to_string()))
}

#[test]
fn crud_roundtrip() {
    let dir = temp_dir("crud");
    // 先手工建"旧版"4 列表（模拟老插件建的库），MemoryStore 首次访问走
    // 整表重建（workspace 键位 + source 列）——迁移路径本身被本用例覆盖
    std::fs::create_dir_all(&dir).unwrap();
    {
        let conn = rusqlite::Connection::open(dir.join("memory.sqlite3")).unwrap();
        conn.execute_batch(
            "CREATE TABLE memories (
                 key TEXT PRIMARY KEY, value TEXT NOT NULL,
                 tags TEXT NOT NULL DEFAULT '', updated_at INTEGER NOT NULL);
             INSERT INTO memories VALUES ('a', 'old', 't', 100);
             INSERT INTO memories VALUES ('b', 'bee', '', 200);",
        )
        .unwrap();
    }
    let store = MemoryStore::new(&dir, test_resolver());

    let rows = store.list("", 200).unwrap();
    assert_eq!(rows.len(), 2, "旧库两行迁入未归属键位");
    assert!(rows.iter().all(|r| r.workspace.is_empty()));
    assert_eq!(rows[0].key, "b", "新→旧排序");
    assert_eq!(rows[0].source, "explicit", "旧行迁移补 explicit 来源");
    assert_eq!(rows[1].updated_at, 100);

    // 经 update 语义没有"新增"，编辑不存在的键返回 false
    assert!(!store.update_value("", "ghost", "v1").unwrap());

    // 过滤：子串命中 value，通配符按字面量
    assert_eq!(store.list("bee", 200).unwrap().len(), 1);
    assert_eq!(store.list("%", 200).unwrap().len(), 0);

    // 编辑：只改 value + 时间，key/source 不动
    assert!(store.update_value("", "a", "new").unwrap());
    let rows = store.list("new", 200).unwrap();
    assert_eq!(rows[0].value, "new");
    assert_eq!(rows[0].key, "a");
    assert!(rows[0].updated_at > 100);

    // 删除 + 清空（全按键位）
    assert!(store.delete("", "a").unwrap());
    assert!(!store.delete("", "a").unwrap(), "重复删除 = 不存在");
    assert_eq!(store.clear_all().unwrap(), 1);
    assert!(store.list("", 200).unwrap().is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn update_validation_rejects_oversize() {
    let dir = temp_dir("validate");
    let store = MemoryStore::new(&dir, test_resolver());
    assert!(store.update_value("", "", "v").is_err());
    assert!(store.update_value("", &"k".repeat(513), "v").is_err());
    assert!(store
        .update_value("", "k", &"v".repeat(256 * 1024 + 1))
        .is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn recall_is_scoped_by_session_workspace() {
    let dir = temp_dir("recall-scope");
    let store = MemoryStore::new(&dir, test_resolver());
    // 项目键位与全局层各一条（同 key，(workspace,key) 主键允许并存）
    store
        .write("F:/ws/a", "k", "project-v", "", "explicit")
        .unwrap();
    store.write("", "k", "global-v", "", "explicit").unwrap();

    // 双层（㉘·A·fix）：归属会话 = 项目行 ∪ 全局行；未归属会话只见全局层
    let got = store.recall(Some("ws-session"), 5);
    assert_eq!(got.len(), 2);
    let mut scopes: Vec<&str> = got.iter().map(|e| e.scope.as_str()).collect();
    scopes.sort();
    assert_eq!(scopes, vec!["global", "project"]);
    assert!(got.iter().any(|e| e.value == "project-v"));
    assert!(got.iter().any(|e| e.value == "global-v"));

    // 未归属 / 无会话：只落全局层，别的项目行不可见
    for sid in [Some("other"), None] {
        let got = store.recall(sid, 5);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].value, "global-v");
        assert_eq!(got[0].scope, "global");
    }

    // 管理面 list 看全量两行（跨键位）
    assert_eq!(store.list("", 200).unwrap().len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn scope_id_and_context_line_follow_attribution() {
    let dir = temp_dir("scope-id");
    let store = MemoryStore::new(&dir, test_resolver());
    // 归属会话：键位身份 = 项目根；上下文行 = 当前项目工作区
    assert_eq!(store.scope_id(Some("ws-session")), "F:/ws/a");
    let line = store.context_line(Some("ws-session")).unwrap();
    assert!(line.contains("F:/ws/a"), "{line}");
    assert!(line.contains("this project"), "{line}");
    // 未归属 / 无会话：空键位 + 无上下文行（不注入）
    assert_eq!(store.scope_id(Some("other")), "");
    assert_eq!(store.scope_id(None), "");
    assert!(store.context_line(Some("other")).is_none());
    assert!(store.context_line(None).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn recall_projects_memory_entries_and_degrades_to_empty() {
    let dir = temp_dir("recall");
    let store = MemoryStore::new(&dir, test_resolver());
    std::fs::create_dir_all(&dir).unwrap();
    // 打不开的目录（用文件路径当目录制造失败）→ 空表不 panic
    let bad = MemoryStore::new(&dir.join("file-not-dir"), test_resolver());
    std::fs::write(dir.join("file-not-dir"), "x").unwrap();
    assert!(bad.recall(None, 5).is_empty());
    {
        let conn = rusqlite::Connection::open(dir.join("memory.sqlite3")).unwrap();
        conn.execute_batch(
            "CREATE TABLE memories (
                 key TEXT PRIMARY KEY, value TEXT NOT NULL,
                 tags TEXT NOT NULL DEFAULT '', updated_at INTEGER NOT NULL,
                 source TEXT NOT NULL DEFAULT 'explicit');
             INSERT INTO memories VALUES ('k', 'v', 'tg', 1, 'explicit');",
        )
        .unwrap();
    }
    let got = store.recall(None, 5);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].key, "k");
    assert_eq!(got[0].value, "v");
    assert_eq!(got[0].tags, "tg");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn limit_is_clamped() {
    let dir = temp_dir("limit");
    let store = MemoryStore::new(&dir, test_resolver());
    {
        std::fs::create_dir_all(&dir).unwrap();
        let conn = rusqlite::Connection::open(dir.join("memory.sqlite3")).unwrap();
        conn.execute_batch(
            "CREATE TABLE memories (
                 key TEXT PRIMARY KEY, value TEXT NOT NULL,
                 tags TEXT NOT NULL DEFAULT '', updated_at INTEGER NOT NULL,
                 source TEXT NOT NULL DEFAULT 'explicit');
             INSERT INTO memories VALUES ('a', 'v', '', 1, 'explicit'),
                                          ('b', 'v', '', 2, 'explicit');",
        )
        .unwrap();
    }
    assert_eq!(store.list("", 0).unwrap().len(), 1, "0 收敛到下限 1");
    assert_eq!(store.list("", 1).unwrap().len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}
