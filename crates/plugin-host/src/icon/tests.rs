use std::path::PathBuf;

use super::IconLoader;

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "locowrk-icon-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const SVG: &[u8] = br#"<svg viewBox="0 0 64 64" xmlns="http://www.w3.org/2000/svg"/>"#;

#[test]
fn svg_loads_as_data_url() {
    let dir = temp_dir("ok");
    std::fs::write(dir.join("icon.svg"), SVG).unwrap();
    let url = IconLoader::new().load(&dir, "icon.svg").expect("svg loads");
    assert!(url.starts_with("data:image/svg+xml;base64,"), "{url}");
}

/// Fallback 矩阵（§7）：文件不存在 / 格式不支持 / 超限 / 穿越一律 None。
#[test]
fn fallback_matrix_all_rejected_as_none() {
    let loader = IconLoader::new();
    let dir = temp_dir("fallback");

    // 图标文件不存在
    assert!(loader.load(&dir, "missing.svg").is_none());
    // 格式不支持（gif/jpeg/ico，§4.1）
    std::fs::write(dir.join("x.gif"), b"GIF").unwrap();
    std::fs::write(dir.join("x.jpg"), b"JPEG").unwrap();
    assert!(loader.load(&dir, "x.gif").is_none());
    assert!(loader.load(&dir, "x.jpg").is_none());
    // 超过 1MB
    std::fs::write(dir.join("big.svg"), vec![b'a'; 1024 * 1024 + 1]).unwrap();
    assert!(loader.load(&dir, "big.svg").is_none());
    // 路径穿越
    std::fs::write(temp_dir("outside").join("evil.svg"), SVG).unwrap();
    assert!(
        loader
            .load(&dir, "../locowrk-icon-outside/evil.svg")
            .is_none()
    );
    // icon 字段本身是绝对路径也逃不出目录校验
    let abs = temp_dir("outside").join("evil.svg");
    assert!(loader.load(&dir, abs.to_str().unwrap()).is_none());
}

/// 缓存生效：文件被删后第二次读取仍命中缓存（plugin_list 高频调用不读盘）。
#[test]
fn second_load_is_cached() {
    let dir = temp_dir("cache");
    std::fs::write(dir.join("icon.png"), b"\x89PNG").unwrap();
    let loader = IconLoader::new();
    let first = loader.load(&dir, "icon.png").unwrap();
    std::fs::remove_file(dir.join("icon.png")).unwrap();
    assert_eq!(
        loader.load(&dir, "icon.png").as_deref(),
        Some(first.as_str())
    );
}
