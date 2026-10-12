//! 事实卡解析簇单测（MR 稿 §5 提案 A · 后审计队列 Q3 · v2.186）：两段切分的顺序
//! 承重、三道降级口径、四道钳，以及发给模型的要求文本里那些**必须真的存在**的契约串。
//!
//! 跨簇引用（`parse_drafts`、`run_summary`）不是越界而是必须：「卡段不许被总结吞
//! 掉」这条判据只有同时看两个消费点才成立——它们共用 `mod.rs` 里 `split_summary_section`
//! 的剥段顺序，只测一侧永远抓不到另一侧被污染。

use super::*;
use crate::manager::{parse_drafts, run_summary};
use crate::material::{MaterialItem, build_items};
use session_store::SessionEvent;

/// 三项材料（骨架的 `event_range` 铺满 `[0,2]`）：本簇用例只关心"数组部分照常
/// 解析"，材料内容不参与断言。
fn three() -> Vec<MaterialItem> {
    let events = vec![
        SessionEvent::UserMessage {
            content: "第一轮：查问题".into(),
            parts: Vec::new(),
        },
        SessionEvent::AssistantMessage {
            content: "好的，先看配置".into(),
            tool_calls: vec![],
        },
        SessionEvent::UserMessage {
            content: "第二轮：改代码".into(),
            parts: Vec::new(),
        },
    ];
    build_items(&events, 0)
}

/// 把一段卡挂到"数组 + 总结"骨架尾部（契约形状：数组 → `===SUMMARY===` →
/// `===FACTS===`）。
fn with_facts(facts: &str) -> String {
    format!(
        "[{{\"title\":\"a\",\"summary\":\"s\",\"event_range\":[0,2]}}]\n===SUMMARY===\n本轮结论是共享 runtime。\n===FACTS===\n{facts}"
    )
}

/// **卡段不许被总结吞掉**（本簇最要紧的一条）：`run_summary` 取的是
/// `===SUMMARY===` 之后的**全部**文本，所以 FACTS 必须在 SUMMARY 之前剥掉——
/// 否则标记和 JSON 一起进总结正文（还会占掉 `RUN_SUMMARY_MAX_CHARS` 的额度）。
///
/// 判据用**等价**而不是字面：同一份骨架带卡段 / 不带卡段，总结必须逐字节相同。
/// 只断"总结里不含 `===FACTS===`"是不够的——顺序打反时它照样绿（总结会以
/// `===FACTS===` 开头，那也是一段文本），等价档才抓得住"多出来那段根本没进"。
#[test]
fn facts_section_below_the_summary_is_not_swallowed_by_it() {
    let bare = "[{\"title\":\"a\",\"summary\":\"s\",\"event_range\":[0,2]}]\n===SUMMARY===\n本轮结论是共享 runtime。\n";
    let raw = with_facts(r#"[{"key":"build.cmd","value":"pnpm tauri dev","type":"project"}]"#);
    assert_eq!(
        run_summary(&raw),
        run_summary(bare),
        "有无卡段都必须给出同一份总结正文"
    );
    let sum = run_summary(&raw).expect("总结仍在");
    assert!(!sum.contains("===FACTS==="), "标记不许进总结: {sum}");
    assert!(!sum.contains("build.cmd"), "卡的内容不许进总结: {sum}");
    // 数组面同样只看卡段之前：`extract_array` 的首尾定位不能被卡里的 `[`/`]` 带跑
    assert_eq!(parse_drafts(&raw, &three()).unwrap().len(), 1);
    // 反过来：**卡段写在总结之前是不容错的**（这条形状是实测出来的，不是我推测的
    // ——第一版我按"两处消费各得其所"断言它仍取到总结，实测 `None`）。FACTS 先剥
    // 会把后面的 SUMMARY 一起带走 ⇒ 总结丢失、章节仍正常。所以**契约顺序是承重的**，
    // 这正是要求文本里明写「段落总结之后」的理由（`fact_request` 那条判据盯着那句）。
    // 真机若发现模型普遍写反，再谈解析层容错——本批不顺手做（已登记队列 §5）。
    let flipped = "[{\"title\":\"a\",\"summary\":\"s\",\"event_range\":[0,2]}]\n===FACTS===\n[]\n===SUMMARY===\n本轮结论是共享 runtime。\n";
    assert_eq!(run_summary(flipped), None, "反序时总结被 FACTS 一起带走");
    assert_eq!(
        parse_drafts(flipped, &three()).unwrap().len(),
        1,
        "但章节不受影响：反序最坏的损失就是没有总结"
    );
}

/// 「产不出就是没产」的一档：`limit = 0`（开关关掉时宿主传 0）连解析都不做、
/// 没有卡段 = 空、垃圾输出 = 空。这三档必须**都返回空而不是 Err**——卡是整理的
/// 副产物，格式契约没被满足时不许把已经成功的整理拖倒（`organize/facts.rs` 头注
/// 第 3 条同一口径）。
#[test]
fn fact_cards_produce_nothing_when_there_is_nothing_to_parse() {
    let one = r#"[{"key":"k","value":"v","type":"project"}]"#;
    assert!(fact_cards(&with_facts(one), 0).is_empty(), "limit=0 不解析");
    assert!(
        fact_cards("[{...}]\n===SUMMARY===\n总结。\n", 3).is_empty(),
        "没有卡段 = 正常情况，不记成丢了 N 张"
    );
    for junk in [
        "我不会",
        "[]",
        "这里有个数组 [1,2] 但不是 JSON",
        "{}",
        "[1,2]",
    ] {
        assert!(
            fact_cards(&with_facts(junk), 3).is_empty(),
            "垃圾档 {junk:?}"
        );
    }
    // 合法但空数组 = 空（模型"这次确实没什么可记"的正常回答）
    assert!(fact_cards(&with_facts("[]"), 3).is_empty());
}

/// 解析层的四道钳：类型白名单、空 key / 空 value、**标识符面**压平、value 字符
/// 上限，外加条数上限（设置的 `factCardMax` 与解析层硬顶 20 是**两道不同的钳**）。
#[test]
fn fact_cards_filter_flatten_and_clamp() {
    let raw = with_facts(
        r#"[{"key":"a.b","value":"项目事实","type":"project","tags":"构建  命令"},
           {"key":"   ","value":"空键","type":"user"},
           {"key":"c.d","value":"   ","type":"user"},
           {"key":"e.f","value":"来历不明","type":"misc"},
           {"key":"g.h","value":"多行\n正文保留","type":"reference"},
           {"key":"bad\nkey","value":"  首尾空白  ","type":"user"}]"#,
    );
    let got = fact_cards(&raw, 9);
    assert_eq!(
        got.iter().map(|c| c.key.as_str()).collect::<Vec<_>>(),
        vec!["a.b", "g.h", "bad key"],
        "白名单外的类型整条丢（归层不许有来历不明的落点）、空 key/空 value 不留"
    );
    assert_eq!(
        got[0].tags, "构建 命令",
        "tags 压成一行（进 md frontmatter）"
    );
    // ⚠ 判据自纠：value 的换行**不压平**。我第一版按 key/tags 的处理断言它也被压，
    // 实测未压才发现两者本来就不该同档——key 是标识符（md 文件名、索引行、主键成分），
    // tags 是单行元数据，而 value 是正文：sqlite 与人类层 md 都容得多行，压平反而
    // 把"一条两点式的事实"折成一行难读的话。这里钉的是**正文内换行保留 + 两端 trim**。
    assert_eq!(got[1].value, "多行\n正文保留");
    assert!(got[1].value.contains('\n'), "正文换行保留");
    assert_eq!(got[2].value, "首尾空白", "两端空白 trim");
    assert_eq!(got[0].card_type, "project", "type 原样留给调用方归层");

    let fat = with_facts(&format!(
        r#"[{{"key":"fat","value":"{}","type":"project"}}]"#,
        "长".repeat(FACT_VALUE_MAX_CHARS + 50)
    ));
    assert_eq!(
        fact_cards(&fat, 3)[0].value.chars().count(),
        FACT_VALUE_MAX_CHARS,
        "value 钳到 {FACT_VALUE_MAX_CHARS} 字符（提示词要 500 字，这里是防灌爆的硬档）"
    );

    let many = {
        let mut s = String::from("[");
        for i in 0..30 {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(r#"{{"key":"k{i}","value":"v","type":"user"}}"#));
        }
        s.push(']');
        s
    };
    assert_eq!(fact_cards(&with_facts(&many), 5).len(), 5, "设置上限生效");
    assert_eq!(
        fact_cards(&with_facts(&many), 999).len(),
        20,
        "解析层硬顶兜住「模型一次灌回几百张」"
    );
}

/// 要求文本 = 发给模型的**契约**：上限要说给它、四个类型要解释给它、标记要写清在
/// 总结之后、既有键清单要带层标列给它。这里钉的每个串都必须真的出现在文本里
/// （串先与本文件的实现核对过，不许照想象写）。
#[test]
fn fact_request_states_the_contract_and_the_existing_keys() {
    let empty = fact_request(&[], 3);
    assert!(empty.starts_with("\n\n"), "拼在转录尾部，不许与正文粘连");
    assert!(empty.contains("最多 3 张"), "上限要告诉模型");
    assert!(empty.contains("===FACTS==="), "标记本身要写给模型");
    assert!(empty.contains("段落总结之后"), "段的先后顺序是解析的承重");
    for t in FACT_TYPES {
        assert!(empty.contains(t), "类型 {t} 必须在要求里解释");
    }
    assert!(empty.contains("（暂无）"), "无既有键时明确说暂无，不是不提");

    let listed = fact_request(
        &["build.cmd (project)".into(), "user.lang (global)".into()],
        2,
    );
    assert!(listed.contains("build.cmd (project)"), "清单行原样进文本");
    assert!(listed.contains("user.lang (global)"));
    assert!(!listed.contains("（暂无）"), "有货时不许再写暂无");
    assert!(
        listed.contains("- build.cmd (project)"),
        "清单行带列表前缀，模型才读得出那是清单正文"
    );
    // `max = 0` 时**清单段整体不出现**（宿主那侧 `fact_limit > 0` 才调用本函数，
    // 这里再兜一道：真传 0 也不许把既有键塞进去——"最多 0 张"后面跟一张清单是
    // 自相矛盾的指令）
    assert_eq!(
        fact_request(&["x (global)".into()], 0),
        fact_request(&[], 0)
    );
}
