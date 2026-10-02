//! experiences 单测：BLOB 往返 / 写入检索 / 维度过滤 / top-k 排序 /
//! 项目键位隔离（㉘）/ 删除清空 / 经历块注入由 memory_inject 侧覆盖。

use super::*;

fn tmp_db(tag: &str) -> (std::path::PathBuf, ExperienceDb) {
    let dir = std::env::temp_dir().join(format!("locowrk-exp-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("experiences.sqlite3");
    (path.clone(), ExperienceDb::open(&path).unwrap())
}

#[test]
fn blob_roundtrip() {
    let v = vec![0.25f32, -1.5, 3.0e-7, f32::INFINITY];
    assert_eq!(blob_to_vec(&vec_to_blob(&v)), v);
    // 非整倍长度：chunks_exact 截尾不 panic
    assert!(blob_to_vec(&[0, 0, 0]).is_empty());
}

#[tokio::test]
async fn insert_search_rank_and_dim_filter() {
    let (_p, db) = tmp_db("search");
    db.insert("decided to use rusqlite", &[1.0, 0.0], "s1", "")
        .unwrap();
    db.insert("talked about http proxies", &[0.0, 1.0], "s2", "")
        .unwrap();
    db.insert("old-dimension row", &[1.0, 0.0, 0.0], "s3", "")
        .unwrap();
    db.insert("zero vector row", &[0.0, 0.0], "s4", "").unwrap();
    let hits = db.search(&[1.0, 0.0], 5, "").unwrap();
    // 维度不符（s3）与零向量（s4）不入列；命中 s1
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].content, "decided to use rusqlite");
    assert_eq!(hits[0].session_id, "s1");

    // top-k 排序：查询与 s1 同向、与 s2 正交
    let hits = db.search(&[0.9, 0.1], 1, "").unwrap();
    assert_eq!(hits[0].content, "decided to use rusqlite");
    assert_eq!(db.count().unwrap(), 4);
}

#[test]
fn search_is_scoped_by_workspace() {
    let (_p, db) = tmp_db("scope");
    db.insert("rust trait objects", &[1.0, 0.0], "s1", "F:/ws/a")
        .unwrap();
    db.insert("vue reactivity", &[1.0, 0.0], "s2", "F:/ws/b")
        .unwrap();
    db.insert("unfiled note", &[1.0, 0.0], "s3", "").unwrap();

    // 各键位只召回自己的经历：a 看不到 b / 未归属
    let a = db.search(&[1.0, 0.0], 10, "F:/ws/a").unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(a[0].content, "rust trait objects");

    let b = db.search(&[1.0, 0.0], 10, "F:/ws/b").unwrap();
    assert_eq!(b.len(), 1);
    assert_eq!(b[0].content, "vue reactivity");

    let unfiled = db.search(&[1.0, 0.0], 10, "").unwrap();
    assert_eq!(unfiled.len(), 1);
    assert_eq!(unfiled[0].content, "unfiled note");

    // 管理面（list_recent）仍看全量
    assert_eq!(db.list_recent(10).unwrap().len(), 3);
}

#[tokio::test]
async fn list_delete_clear() {
    let (_p, db) = tmp_db("crud");
    db.insert("a", &[1.0], "s1", "").unwrap();
    db.insert("b", &[1.0], "s2", "").unwrap();
    let rows = db.list_recent(10).unwrap();
    assert_eq!(rows.len(), 2);
    let id_b = rows.iter().find(|r| r.content == "b").unwrap().id;
    assert!(db.delete(id_b).unwrap());
    assert!(!db.delete(999_999).unwrap());
    assert_eq!(db.count().unwrap(), 1);
    assert_eq!(db.clear_all().unwrap(), 1);
    assert_eq!(db.count().unwrap(), 0);
}

#[test]
fn empty_query_or_topk_is_empty() {
    let (_p, db) = tmp_db("empty");
    assert!(db.search(&[], 5, "").unwrap().is_empty());
    assert!(db.search(&[1.0], 0, "").unwrap().is_empty());
}
