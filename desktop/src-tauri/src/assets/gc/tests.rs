use super::*;
use std::time::{Duration, SystemTime};

fn tmp(tag: &str) -> std::path::PathBuf {
    let d = crate::testutil::work_dir(&format!("gc-{}", tag));
    d
}

fn put_asset(root: &Path, dir: &str, id: &str, body: &[u8]) -> std::path::PathBuf {
    let d = root.join(dir);
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join(format!("{id}.bin"));
    std::fs::write(&p, body).unwrap();
    p
}

#[test]
fn referenced_assets_survive_orphans_expire() {
    let root = tmp("run");
    let sessions = root.join("sessions");
    let subs = root.join("sub-transcripts");
    std::fs::create_dir_all(&sessions).unwrap();
    std::fs::create_dir_all(&subs).unwrap();
    let assets = root.join("assets");

    let kept = put_asset(&assets, "images", "sha-kept", b"keep");
    let orphan = put_asset(&assets, "images", "sha-orphan", b"drop");
    // 引用面：sessions jsonl + 子转录各扫一处
    std::fs::write(sessions.join("s1.jsonl"), r#"{"id":"sha-kept"}"#).unwrap();
    std::fs::write(subs.join("c1.jsonl"), "other text").unwrap();

    // gc_days = 0（全部立即过期）：孤儿删、被引用留
    let r = run_gc(&assets, &sessions, &subs, 0);
    assert_eq!(
        r,
        GcReport {
            scanned: 2,
            removed: 1
        }
    );
    assert!(kept.exists());
    assert!(!orphan.exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn grace_period_spares_fresh_orphans() {
    let root = tmp("grace");
    let sessions = root.join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let assets = root.join("assets");
    let fresh = put_asset(&assets, "files", "sha-fresh", b"x");
    let subs = root.join("sub-transcripts");

    // 宽限 7 天：新文件在当前时钟下不动
    let r = run_gc(&assets, &sessions, &subs, 7);
    assert_eq!(r.removed, 0);
    assert!(fresh.exists());

    // 时钟拨到 8 天后 → 同一文件过期删（时钟注入，不碰平台 mtime API）
    let later = SystemTime::now() + Duration::from_secs(8 * 24 * 3600);
    let r = run_gc_at(&assets, &sessions, &subs, 7, later);
    assert_eq!(r.removed, 1);
    assert!(!fresh.exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn absent_dirs_are_empty_scan() {
    let root = tmp("absent");
    let r = run_gc(&root.join("assets"), &root.join("s"), &root.join("t"), 0);
    assert_eq!(
        r,
        GcReport {
            scanned: 0,
            removed: 0
        }
    );
    let _ = std::fs::remove_dir_all(&root);
}
