//! 渲染 ↔ 解析往返（自 `tests.rs` 搬入，用例本体逐字未改）：frontmatter 各行的
//! 写法与读回兜底。这片是"人类层可编辑"的地基——人改完文件机器还读得回来。

use super::*;

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
