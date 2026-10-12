//! 粘性规则单测：匹配语义（工具级 / 精确 / 前缀 token 边界）/ 存储往返
//! （隔离临时根，不碰真实数据目录）/ 坏 JSON 当空 / 幂等与清除。

use super::{rule_matches, PermissionRule, PermissionRuleStore, RuleVerdict};
use plugin_api::memlayout::fnv1a64;

fn rule(tool: &str, content: Option<&str>) -> PermissionRule {
    PermissionRule {
        tool: tool.into(),
        content: content.map(str::to_string),
    }
}

fn store(tag: &str) -> PermissionRuleStore {
    let dir = crate::testutil::work_dir(&format!("rules-test-{}", tag));
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
    assert!(!s.verdicts(ws, "shell", Some("node server.js")).allowed);

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
    assert!(s.verdicts(ws, "shell", Some("node server.js")).allowed);
    assert!(!s.verdicts(ws, "shell", Some("node other.js")).allowed);
    assert!(s.verdicts(ws, "fs_write", None).allowed);

    // 幂等：同条重复添加不增长
    s.add_allow(ws, "shell", Some("node server.js".into()))
        .unwrap();
    assert_eq!(s.list_all()[0].allow, 2);

    // 不同工作区互不可见
    assert_eq!(s.list_all().len(), 1);

    s.clear(ws).unwrap();
    assert_eq!(s.list_all().len(), 0);
    assert!(!s.verdicts(ws, "shell", Some("node server.js")).allowed);
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
    assert!(!s.verdicts(ws, "shell", Some("anything")).allowed);
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
    assert!(s.verdicts(ws, "shell", Some("rm -rf build")).allowed);
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
    assert!(s.verdicts(ws, "shell", Some("rm -rf build")).denied);
    assert!(!s.verdicts(ws, "shell", Some("npm test")).denied);
    let _ = std::fs::remove_dir_all(s.dir());
}

// ---- M-18 等价证明：HEAD 原文当参考实现（批 F-24a）------------------------
//
// 三份参考实现（本文件两份 + `tests_approval/m18_equiv.rs` 一份）都是从
// `git show HEAD:desktop/src-tauri/src/permission_rules.rs` 逐字抄来的
// **被删/被改原文**，只把方法改成自由函数（本模块是 `permission_rules` 的子
// 模块，`use super::*` 拿得到私有的 `read`/`io`）。它们存在的唯一理由 = 让
// 「零行为变化」这句话有机器可判的真源；别当第二套实现来维护，也别删。

/// M-18e③ 改前原文：前缀档用 `format!("{prefix} ")` + `starts_with`
/// （改后生产版换成 `strip_prefix` + `is_some_and`，少一次堆分配）。
fn ref_rule_matches_old(rule: &PermissionRule, tool: &str, subject: Option<&str>) -> bool {
    if rule.tool != tool {
        return false;
    }
    let Some(content) = &rule.content else {
        return true;
    };
    let Some(subject) = subject else {
        return false;
    };
    match content.strip_suffix(":*") {
        Some(prefix) => subject == prefix || subject.starts_with(&format!("{prefix} ")),
        None => subject == content,
    }
}

/// M-18b 改前原文之一：决策表 ③ 级（自己读一遍文件）。
fn ref_denies_old(
    s: &PermissionRuleStore,
    workspace: &str,
    tool: &str,
    subject: Option<&str>,
) -> bool {
    let _g = s.io.lock().expect("permission rules io poisoned");
    s.read(workspace)
        .deny
        .iter()
        .any(|r| ref_rule_matches_old(r, tool, subject))
}

/// M-18b 改前原文之二：决策表 ⑤ 级（**又**读一遍同一个文件——M-18b 消掉的
/// 就是这第二遍 IO 与「两次读之间被人改 ⇒ ③⑤看到两个版本」的不自洽）。
fn ref_allows_old(
    s: &PermissionRuleStore,
    workspace: &str,
    tool: &str,
    subject: Option<&str>,
) -> bool {
    let _g = s.io.lock().expect("permission rules io poisoned");
    s.read(workspace)
        .allow
        .iter()
        .any(|r| ref_rule_matches_old(r, tool, subject))
}

/// M-18e③ 等值矩阵：`rule_matches` 与 HEAD 原文逐格相等。格子按
/// 「content 形态 × subject 形态 × 工具名」铺开，重点覆盖 `strip_prefix`
/// 改写最容易出错的四处 = 词边界（`npm testify` 不许命中 `npm test:*`）、
/// 空前缀（`":*"` 对任何以空格开头的 subject 都命中）、多字节（中文命令，
/// `strip_prefix` 不按 char boundary 匹配时返回 None 而不是 panic）、
/// 制表符分隔（改前只认空格 ⇒ 改后也必须只认空格）。
#[test]
fn rule_matches_matches_format_prefix_reference() {
    let contents: &[Option<&str>] = &[
        None,
        Some("npm test"),
        Some("npm test:*"),
        Some(":*"),
        Some(""),
        Some("修复:*"),
        Some("npm testx:*"),
        Some("a b:*"),
        Some("npm test:*:*"),
    ];
    let subjects: &[Option<&str>] = &[
        None,
        Some(""),
        Some("npm test"),
        Some("npm test --watch"),
        Some("npm testify"),
        Some("npm run test"),
        Some(" npm test"),
        Some("npm test\t--watch"),
        Some("npm test:"),
        Some("npm test:*"),
        Some("修复"),
        Some("修复 --all"),
        Some("修复x"),
        Some("a b"),
        Some("a b c"),
        Some("a bc"),
    ];
    let (mut n, mut hit, mut miss) = (0usize, 0usize, 0usize);
    for tool in ["shell", "other"] {
        for c in contents {
            let r = rule("shell", *c); // 规则的工具名恒 shell，靠 tool 参数测不匹配档
            for s in subjects {
                let got = rule_matches(&r, tool, *s);
                let want = ref_rule_matches_old(&r, tool, *s);
                assert_eq!(got, want, "content={c:?} tool={tool} subject={s:?}");
                n += 1;
                hit += got as usize;
                miss += (!got) as usize;
            }
        }
    }
    assert_eq!(
        n,
        2 * contents.len() * subjects.len(),
        "矩阵必须真的跑满（防迭代器写错导致空转 = 零证明力）"
    );
    // 阳性对照：矩阵不许退化成一色（全命中或全不命中都说明格子铺错了）
    assert!(
        hit > 0 && miss > 0,
        "矩阵退化：命中 {hit} / 不命中 {miss}，两者都必须非零"
    );
}

/// M-18b 等值：`verdicts`（一次读盘两判定）与 HEAD 的 `denies`/`allows`
/// （两次读盘）在各种**文件状态**下逐格相等。状态覆盖 = 无文件 / 空规则集 /
/// 只 allow（精确·工具级·前缀级）/ 只 deny / 两者都命中 / 坏 JSON / 版本
/// 不识别——后两态是「宁缺勿假」那条既有语义，改写不许把它弄丢。
#[test]
fn verdicts_matches_separate_denies_allows() {
    let s = store("equiv-verdicts");
    let ws = "ws-equiv";
    let states: &[(&str, &str)] = &[
        ("无文件", ""),
        (
            "空规则集",
            r#"{"version":1,"workspace":"ws-equiv","allow":[],"deny":[]}"#,
        ),
        (
            "只 allow 精确",
            r#"{"version":1,"allow":[{"tool":"shell","content":"npm test"}],"deny":[]}"#,
        ),
        (
            "只 allow 工具级",
            r#"{"version":1,"allow":[{"tool":"fs_write"}],"deny":[]}"#,
        ),
        (
            "只 allow 前缀级",
            r#"{"version":1,"allow":[{"tool":"shell","content":"npm test:*"}],"deny":[]}"#,
        ),
        (
            "只 deny",
            r#"{"version":1,"allow":[],"deny":[{"tool":"shell","content":"rm -rf build"}]}"#,
        ),
        (
            "两者都命中同一条",
            r#"{"version":1,"allow":[{"tool":"shell","content":"npm test"}],"deny":[{"tool":"shell","content":"npm test"}]}"#,
        ),
        (
            "allow 与 deny 各命中不同探测",
            r#"{"version":1,"allow":[{"tool":"shell","content":"npm test:*"}],"deny":[{"tool":"shell","content":"git push:*"}]}"#,
        ),
        ("坏 JSON", "{not json"),
        (
            "版本不识别",
            r#"{"version":99,"allow":[{"tool":"shell"}],"deny":[{"tool":"shell"}]}"#,
        ),
        (
            "缺 version 字段",
            r#"{"allow":[{"tool":"shell"}],"deny":[]}"#,
        ),
    ];
    let probes: &[(&str, Option<&str>)] = &[
        ("shell", None),
        ("shell", Some("npm test")),
        ("shell", Some("npm test --watch")),
        ("shell", Some("npm testify")),
        ("shell", Some("rm -rf build")),
        ("shell", Some("git push origin")),
        ("fs_write", None),
        ("fs_write", Some("anything")),
        ("other", None),
    ];
    std::fs::create_dir_all(s.dir()).unwrap();
    let path = s.dir().join(format!("{:08x}.json", fnv1a64(ws.as_bytes())));
    let (mut n, mut denied_n, mut allowed_n) = (0usize, 0usize, 0usize);
    for (tag, text) in states {
        if text.is_empty() {
            let _ = std::fs::remove_file(&path);
        } else {
            std::fs::write(&path, text).unwrap();
        }
        for (tool, subject) in probes {
            let got = s.verdicts(ws, tool, *subject);
            let want = RuleVerdict {
                denied: ref_denies_old(&s, ws, tool, *subject),
                allowed: ref_allows_old(&s, ws, tool, *subject),
            };
            assert_eq!(got, want, "状态「{tag}」 tool={tool} subject={subject:?}");
            n += 1;
            denied_n += got.denied as usize;
            allowed_n += got.allowed as usize;
        }
    }
    assert_eq!(n, states.len() * probes.len(), "矩阵必须真的跑满");
    // 阳性对照：deny 面与 allow 面都必须真的被命中过（否则某一侧的等值
    // 断言是空转——例如 deny 全 false 时「denied 字段写反」也照样绿）
    assert!(
        denied_n > 0 && allowed_n > 0,
        "矩阵退化：denied 命中 {denied_n} / allowed 命中 {allowed_n}，两者都必须非零"
    );
    let _ = std::fs::remove_dir_all(s.dir());
}
