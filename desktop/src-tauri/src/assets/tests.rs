use super::*;

fn store(tag: &str) -> (AssetStore, std::path::PathBuf) {
    let root = std::env::temp_dir().join(format!("locowrk-asset-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    (AssetStore::new(root.join("assets")), root)
}

fn limits() -> validate::Limits {
    validate::Limits {
        max_dimension: 2048,
        max_size_mb: 5,
        image_formats: vec!["png".into(), "jpg".into(), "webp".into(), "gif".into()],
        file_formats: vec!["pdf".into()],
    }
}

/// 最小 PNG（100×50，IHDR 头合法）。
fn png() -> Vec<u8> {
    let mut v = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
    v.extend_from_slice(&[0, 0, 0, 13]);
    v.extend_from_slice(b"IHDR");
    v.extend_from_slice(&100u32.to_be_bytes());
    v.extend_from_slice(&50u32.to_be_bytes());
    v.extend_from_slice(&[8, 6, 0, 0, 0]);
    v
}

#[test]
fn put_dedupes_by_content_and_reads_back() {
    let (store, root) = store("put");
    let a = store.put(&png(), "image/png", "a.png", &limits()).unwrap();
    let b = store.put(&png(), "image/png", "b.png", &limits()).unwrap();
    // 内容寻址去重：同字节 = 同 id（name 只作展示）
    assert_eq!(a.id, b.id);
    assert_eq!(a.mime, "image/png");

    let url = store.data_url(&a.id, &a.mime).unwrap();
    assert!(url.starts_with("data:image/png;base64,"), "{url}");
    assert!(store.path_of(&a.id, &a.mime).unwrap().exists());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn rejects_rejected_format_and_returns_error_on_missing() {
    let (store, root) = store("reject");
    assert!(store
        .put(b"not an image", "image/png", "x.png", &limits())
        .is_err());
    // 悬空引用：读取如实报错（不静默丢）
    assert!(store.data_url("deadbeef", "image/png").is_err());
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn sha256_hex_stable() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
