//! `meta` 的等价测试：参考实现 = 十份插件里**被删掉的手写形态**原样抄（`ref_zh_en`
//! 抄 `fn zh_en`，`ref_tail` 抄 `describe()` 尾块的字面赋值），逐字节对账 `describe`
//! JSON。先例 = 批 F-1 `now_ms` / 批 F-2 `clip` / 批 F-21a `arg_u64` / 批 F-21b `netsafe`。
//!
//! 分工：结构面（这十个字段怎么填）在本文件证；**文案面**（十条 zh/en 文本有没有被
//! 机械重写改漂）由门禁 `meta_texts` 拿 `git show HEAD:` 的原始实参文本逐字回比，
//! 两条腿合起来才覆盖"行为逐字等价"这句话。
use std::collections::BTreeMap;

use super::{official_meta, zh_en};
use crate::manifest::{
    Author, LocalizedOrString, LocalizedString, PluginKind, PluginManifest, PluginOrigin,
};
use crate::tool::{Exposure, SideEffect, ToolSpec};

/// 被上收掉的 `fn zh_en` 原文（十份实测逐字节相同）。
fn ref_zh_en(zh: &str, en: &str) -> LocalizedString {
    LocalizedString(BTreeMap::from([
        ("zh-CN".to_owned(), zh.to_owned()),
        ("en-US".to_owned(), en.to_owned()),
    ]))
}

/// 被上收掉的 `describe()` 尾块字面赋值原文（十个插件各写一遍的那一段）。
fn ref_tail(m: &mut PluginManifest, name_zh: &str, name_en: &str, desc_zh: &str, desc_en: &str) {
    m.display_name = Some(ref_zh_en(name_zh, name_en));
    m.description = Some(LocalizedOrString::Localized(ref_zh_en(desc_zh, desc_en)));
    m.icon = Some("assets/icon.svg".into());
    m.author = Some(Author {
        name: "Locowrk Team".into(),
        url: None,
    });
    m.origin = PluginOrigin::Official;
    m.homepage = None;
    m.license = Some("MIT".into());
}

fn base(id: &str) -> PluginManifest {
    PluginManifest {
        id: id.into(),
        version: "0.1.0".into(),
        abi_version: crate::PLUGIN_ABI_VERSION,
        kind: PluginKind::Tool,
        tools: vec![ToolSpec {
            name: "t".into(),
            description: "d".into(),
            schema: serde_json::json!({ "type": "object" }),
            side_effect: SideEffect::Pure,
            exposure: Exposure::Always,
        }],
        ..Default::default()
    }
}

/// `..Default::default()` 的默认值必须与被删尾块的「未填态」一致：七个可选元数据
/// 字段全 None、origin 为 Community（§2.4 安全默认，缺省不伪造官方徽章）。
/// 这条独立于上面两条等值测试——那两条里 `official_meta` 会把七个字段全覆盖，
/// 默认值写错了也照绿，所以默认面要单独钉。
#[test]
fn default_leaves_the_metadata_tail_empty_and_community() {
    let d = PluginManifest::default();
    assert_eq!(d.display_name, None);
    assert_eq!(d.description, None);
    assert_eq!(d.icon, None);
    assert_eq!(d.author, None);
    assert_eq!(d.homepage, None);
    assert_eq!(d.license, None);
    assert_eq!(d.origin, PluginOrigin::Community, "缺省必须 community");
    assert_eq!(d.abi_version, crate::PLUGIN_ABI_VERSION);
    assert!(d.id.is_empty() && d.tools.is_empty(), "核心字段是占位值");
    // 阳性对照：同一默认值经 official_meta 之后 origin 才翻成 Official
    let mut m = base("tool-default");
    assert_eq!(m.origin, PluginOrigin::Community);
    official_meta(&mut m, ("甲", "A"), ("乙", "B"));
    assert_eq!(m.origin, PluginOrigin::Official);
}

/// 三组形态不同的文案：ASCII / 中文带全角标点 / 长句含斜杠分隔。
fn texts() -> [(&'static str, &'static str, &'static str, &'static str); 3] {
    [
        ("回显", "Echo", "回显输入文本", "Echoes the given text back"),
        (
            "持久记忆",
            "Persistent Memory",
            "跨会话持久记忆：写 / 读 / 列举 / 搜索 / 删除（写入可机械撤销）",
            "Cross-session memory: write / read / list / search / delete (writes undoable)",
        ),
        (
            "命令终端",
            "Shell",
            "分级命令执行：可证明只读的命令免审批，其余命令进审批卡；无命令级禁令",
            "Tiered command execution: provably read-only commands run without approval; \
             everything else goes to user approval; no command is banned outright",
        ),
    ]
}

#[test]
fn official_meta_describe_json_is_byte_identical_to_the_hand_written_tail() {
    for (i, (nz, ne, dz, de)) in texts().iter().enumerate() {
        let mut a = base(&format!("tool-a{i}"));
        let mut b = base(&format!("tool-b{i}"));
        // id 不同会让 JSON 差一个字段，先把两边 id 对齐再比整串字节
        b.id = a.id.clone();
        ref_tail(&mut a, nz, ne, dz, de);
        official_meta(&mut b, (*nz, *ne), (*dz, *de));
        assert_eq!(
            a.to_describe_json().as_bytes(),
            b.to_describe_json().as_bytes(),
            "[{i}] describe JSON 与手写尾块不等"
        );
    }
}

#[test]
fn official_meta_matches_field_by_field_and_flips_origin_only_when_called() {
    for (nz, ne, dz, de) in texts() {
        let mut ref_m = base("tool-ref");
        let mut got_m = base("tool-ref");
        ref_tail(&mut ref_m, nz, ne, dz, de);
        official_meta(&mut got_m, (nz, ne), (dz, de));
        assert_eq!(ref_m.display_name, got_m.display_name, "display_name 变了");
        assert_eq!(ref_m.description, got_m.description, "description 变了");
        assert_eq!(ref_m.icon, got_m.icon, "icon 变了");
        assert_eq!(ref_m.author, got_m.author, "author 变了");
        assert_eq!(ref_m.license, got_m.license, "license 变了");
        assert_eq!(got_m.origin, PluginOrigin::Official, "官方标识必须显式给出");
    }
    // 阳性对照：不调 official_meta 时 origin 仍是 community（缺省不伪造官方徽章）
    assert_eq!(base("tool-x").origin, PluginOrigin::Community);
}

#[test]
fn zh_en_builds_both_locale_keys_exactly_like_the_deleted_helper() {
    for (zh, en) in [
        ("文件工具", "File Tools"),
        ("MCP 桥接", "MCP Bridge"),
        ("", ""),
    ] {
        assert_eq!(
            zh_en(zh, en).0,
            ref_zh_en(zh, en).0,
            "映射内容变了: {zh}/{en}"
        );
        assert_eq!(zh_en(zh, en).pick("zh-CN"), Some(zh), "中文取不到");
        assert_eq!(zh_en(zh, en).pick("en-US"), Some(en), "英文取不到");
    }
    // 回退链（packaging §2.3）：未知语言落到 en-US，不是崩
    assert_eq!(zh_en("甲", "A").pick("ja-JP"), Some("A"), "缺省回退链变了");
}
