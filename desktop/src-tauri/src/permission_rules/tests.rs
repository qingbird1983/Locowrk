//! 粘性规则单测：匹配语义（工具级 / 精确 / 前缀 token 边界）/ 存储往返
//! （隔离临时根，不碰真实数据目录）/ 坏 JSON 当空 / 幂等与清除。

use super::{rule_matches, PermissionRule, PermissionRuleStore};
use plugin_api::memlayout::fnv1a64;

fn rule(tool: &str, content: Option<&str>) -> PermissionRule {
    PermissionRule {
        tool: tool.into(),
        content: content.map(str::to_string),
    }
}

fn store(tag: &str) -> PermissionRuleStore {
    let dir = std::env::temp_dir().join(format!("locowrk-rules-test-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    PermissionRuleStore::new(&dir)
}

#[test]
fn matching_semantics() {
    // 工具级规则：任何 subject 命中
    assert!(rule_matches(&rule("fs_write", None), "fs_write", None));
    assert!(rule_matches(
        &rule("fs_write", None),
        "fs_write",
        Some("anything")
    ));
    // 工具名不匹配不命中
    assert!(!rule_matches(&rule("fs_write", None), "shell", None));
    // content 规则对无 subject 调用不命中
    assert!(!rule_matches(
        &rule("shell", Some("npm test")),
        "shell",
        None
    ));
    // 精确匹配
    assert!(rule_matches(
        &rule("shell", Some("npm test")),
        "shell",
        Some("npm test")
    ));
    assert!(!rule_matches(
        &rule("shell", Some("npm test")),
        "shell",
        Some("npm run test")
    ));
    // 前缀 `:*`：token 边界（前缀本身或前缀+空格），不吞子串
    let prefix = rule("shell", Some("npm test:*"));
    assert!(rule_matches(&prefix, "shell", Some("npm test")));
    assert!(rule_matches(&prefix, "shell", Some("npm test --watch")));
    assert!(!rule_matches(&prefix, "shell", Some("npm testify")));
    assert!(!rule_matches(&prefix, "shell", Some("npm run test")));
}

#[test]
fn roundtrip_add_count_clear() {
    let s = store("roundtrip");
    let ws = "F:\\some\\project";
    assert_eq!(s.list_all().len(), 0);
    assert!(!s.allows(ws, "shell", Some("node server.js")));

    s.add_allow(ws, "shell", Some("node server.js".into()))
        .unwrap();
    s.add_allow(ws, "fs_write", None).unwrap();
    assert_eq!(
        s.list_all(),
        vec![super::ProjectRulesView {
            workspace: ws.to_string(),
            allow: 2,
            deny: 0,
        }]
    );
    assert!(s.allows(ws, "shell", Some("node server.js")));
    assert!(!s.allows(ws, "shell", Some("node other.js")));
    assert!(s.allows(ws, "fs_write", None));

    // 幂等：同条重复添加不增长
    s.add_allow(ws, "shell", Some("node server.js".into()))
        .unwrap();
    assert_eq!(s.list_all()[0].allow, 2);

    // 不同工作区互不可见
    assert_eq!(s.list_all().len(), 1);

    s.clear(ws).unwrap();
    assert_eq!(s.list_all().len(), 0);
    assert!(!s.allows(ws, "shell", Some("node server.js")));
    let _ = std::fs::remove_dir_all(s.dir());
}

#[test]
fn corrupt_json_reads_as_empty() {
    let s = store("corrupt");
    let ws = "ws-corrupt";
    std::fs::create_dir_all(s.dir()).unwrap();
    let fname = format!("{:08x}.json", fnv1a64(ws.as_bytes()));
    std::fs::write(s.dir().join(&fname), "{not json").unwrap();
    assert_eq!(s.list_all().len(), 0, "坏 JSON = 当空（宁缺勿假）");
    assert!(!s.allows(ws, "shell", Some("anything")));
    // 后续正常写入自愈
    s.add_allow(ws, "fs_write", None).unwrap();
    assert_eq!(s.list_all().len(), 1);
    let _ = std::fs::remove_dir_all(s.dir());
}

#[test]
fn deny_rules_gate_even_without_ui() {
    let s = store("deny");
    let ws = "ws2";
    s.add_allow(ws, "shell", Some("rm -rf build".into()))
        .unwrap();
    assert!(s.allows(ws, "shell", Some("rm -rf build")));
    // 手改 JSON 注入 deny（UI 不产 deny，文件面生效）
    std::fs::create_dir_all(s.dir()).unwrap();
    let entries: Vec<_> = std::fs::read_dir(s.dir())
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
    let text = std::fs::read_to_string(&entries[0]).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    v["deny"] = serde_json::json!([{ "tool": "shell", "content": "rm -rf build" }]);
    std::fs::write(&entries[0], serde_json::to_string_pretty(&v).unwrap()).unwrap();
    assert!(s.denies(ws, "shell", Some("rm -rf build")));
    assert!(!s.denies(ws, "shell", Some("npm test")));
    let _ = std::fs::remove_dir_all(s.dir());
}
