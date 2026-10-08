//! recommend 单测：工具门、价格序、族分散、全未知保底、确定性。

use super::{recommend, RecCandidate};

fn c(id: &str, tool: Option<bool>, in_p: Option<f64>, out_p: Option<f64>, family: Option<&str>) -> RecCandidate {
    RecCandidate {
        id: id.into(),
        tool_call: tool,
        input_price: in_p,
        output_price: out_p,
        family: family.map(str::to_string),
    }
}

#[test]
fn drops_known_no_toolcall_when_better_exists() {
    let list = vec![
        c("a-nontool", Some(false), Some(0.1), Some(0.2), Some("fam-a")),
        c("b-tool", Some(true), Some(0.3), Some(0.6), Some("fam-b")),
        c("c-unknown", None, Some(0.2), Some(0.4), Some("fam-c")),
    ];
    let out = recommend(&list);
    assert!(!out.contains(&"a-nontool".to_string()));
    assert!(out.contains(&"c-unknown".to_string()));
}

#[test]
fn keeps_all_when_nobody_has_toolcall() {
    let list = vec![
        c("a", None, Some(0.1), Some(0.2), Some("f1")),
        c("b", Some(false), Some(0.2), Some(0.4), Some("f2")),
    ];
    assert_eq!(recommend(&list).len(), 2);
}

#[test]
fn price_ascending_unknown_last_family_spread() {
    let list = vec![
        c("cheap-samefam", Some(true), Some(0.05), Some(0.1), Some("deepseek")),
        c("cheap2-samefam", Some(true), Some(0.06), Some(0.1), Some("deepseek")),
        c("mid", Some(true), Some(0.5), Some(1.0), Some("glm")),
        c("priceless", Some(true), None, None, Some("mystery")),
        c("pricey", Some(true), Some(3.0), Some(15.0), Some("claude")),
    ];
    // 未知价格排最后 → 第三个名额被已知价的 pricey 占（cheap2 同族被
    // 族分散跳过）；priceless 出局是「未知最后」的正确代价。
    let out = recommend(&list);
    assert_eq!(out, vec!["cheap-samefam".to_string(), "mid".to_string(), "pricey".to_string()]);
}

#[test]
fn unknown_family_models_do_not_shadow_each_other() {
    let list = vec![
        c("u1", Some(true), Some(0.1), Some(0.1), None),
        c("u2", Some(true), Some(0.2), Some(0.2), None),
        c("u3", Some(true), Some(0.3), Some(0.3), None),
    ];
    assert_eq!(recommend(&list).len(), 3);
}

#[test]
fn no_catalog_hits_falls_back_to_list_order() {
    let list = vec![c("m3", None, None, None, None), c("m1", None, None, None, None), c("m2", None, None, None, None), c("m4", None, None, None, None)];
    assert_eq!(recommend(&list), vec!["m3".to_string(), "m1".to_string(), "m2".to_string()]);
}

#[test]
fn deterministic_on_ties() {
    let list = vec![
        c("b", Some(true), Some(0.1), Some(0.1), Some("f1")),
        c("a", Some(true), Some(0.1), Some(0.1), Some("f2")),
    ];
    assert_eq!(recommend(&list)[0], "a");
}
