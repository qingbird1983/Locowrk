//! 预览协议纯函数单测：URL 编解码往返 / 逃生段拒绝 / MIME 表 / 注入
//! 协同。fsRoots 围栏的端到端（canonicalize 比对）走 preview_scope 集成
//! 用例（临时目录真实 fs 依赖）。

use super::*;

#[test]
fn preview_url_encodes_segments_keeps_slashes() {
    let url = preview_url("F:\\Warehouse\\TITY\\breakout.html");
    assert!(url.starts_with("http://lwpreview.localhost/"));
    assert!(url.contains("/F%3A/Warehouse/TITY/breakout.html"), "{url}");
}

#[test]
fn preview_url_strips_verbatim_prefix() {
    // canonicalize 产物（\\?\F:\...）不得把 "?" 带进 URL（走查实抓）
    let url = preview_url(r"\\?\F:\Warehouse\TITY\breakout.html");
    assert_eq!(
        url,
        "http://lwpreview.localhost/F%3A/Warehouse/TITY/breakout.html"
    );
    assert!(!url.contains("%3F/F%3A"), "{url}");
}

#[test]
fn decode_segments_roundtrips_preview_url() {
    let url = preview_url("F:\\Warehouse\\TITY\\my game (v2).html");
    let raw = url.strip_prefix("http://lwpreview.localhost").unwrap();
    let segs = decode_segments(raw).expect("decodable");
    assert_eq!(segs.join("/"), "F:/Warehouse/TITY/my game (v2).html");
}

#[test]
fn decode_segments_resolves_dotdot_within_stack() {
    let segs = decode_segments("/F%3A/dir/sub/../other.js").expect("decodable");
    assert_eq!(segs.join("/"), "F:/dir/other.js");
}

#[test]
fn decode_segments_rejects_root_escape() {
    // 顶层 .. 直接出栈失败
    assert!(decode_segments("/..").is_none());
    // %2E%2E 解码成 ".." 同样按出栈处理，顶层一样拒绝
    assert!(decode_segments("/%2E%2E/x").is_none());
    // %2F 解码后段内出现分隔符 = 伪装层级，拒绝
    assert!(decode_segments("/..%2Fetc").is_none());
}

#[test]
fn decode_segments_rejects_embedded_separator() {
    // %2F 解码后段内出现分隔符 = 伪装层级，拒绝
    assert!(decode_segments("/F%3A/a%2Fb.html").is_none());
}

#[test]
fn mime_table_covers_renderable_set() {
    assert!(mime_for("HTML").starts_with("text/html"));
    assert_eq!(mime_for("svg"), "image/svg+xml");
    assert_eq!(mime_for("wasm"), "application/wasm");
    assert_eq!(mime_for("exe"), "application/octet-stream");
}

/// 真实 fs 的围栏比对：临时目录内放行、外拒 403、缺失 404。
#[test]
fn scope_check_enforces_root_prefix() {
    let base = crate::testutil::unique_dir("lw-preview-test");
    let inner = base.join("proj");
    std::fs::create_dir_all(&inner).unwrap();
    let file = inner.join("page.html");
    std::fs::write(&file, "<p>hi</p>").unwrap();

    let roots = vec![inner.to_string_lossy().to_string()];
    assert!(scope_check(&roots, &file).is_ok(), "根内放行");

    let outside = base.join("outside.html");
    std::fs::write(&outside, b"x").unwrap();
    assert_eq!(scope_check(&roots, &outside).unwrap_err(), 403);

    let missing = inner.join("missing.html");
    assert_eq!(scope_check(&roots, &missing).unwrap_err(), 404);

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn injected_html_response_contains_bridge() {
    let page = "<!DOCTYPE html><html><head><title>t</title></head></html>";
    let out = bridge::inject_bridge(page);
    assert!(out.contains("__lwPreviewBridge"));
    assert!(out.starts_with("<!DOCTYPE html><html><head><script>"));
}
