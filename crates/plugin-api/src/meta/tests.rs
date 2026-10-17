//! `meta` 的等价测试：参考实现 = 十份插件里**被删掉的手写形态**原样抄（`ref_zh_en`
//! 抄 `fn zh_en`，`ref_tail` 抄 `describe()` 尾块的字面赋值），逐字节对账 `describe`
//! JSON。先例 = 批 F-1 `now_ms` / 批 F-2 `clip` / 批 F-21a `arg_u64` / 批 F-21b `netsafe`。
//!
//! 分工：结构面（这十个字段怎么填）在本文件证；**文案面**（十条 zh/en 文本有没有被
//! 机械重写改漂）由门禁 `meta_texts` 拿 `git show HEAD:` 的原始实参文本逐字回比，
//! 两条腿合起来才覆盖"行为逐字等价"这句话。
use std::collections::BTreeMap;

use super::{drift_against_file, drift_of_describe, manifest_drift, official_meta, zh_en};
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

// ---------------------------------------------------------- manifest_drift（台账 T11）

/// 磁盘那份的"同源态"：除 `version` 外全部照抄代码那份。
/// `version` 故意不同——它是**唯一不该比**的字段（构建脚本按 PLAN 清单注入两侧，
/// 源码里那份只是占位），所以这条同时是"不比 version"的实证 + 下面几条的阴性对照。
fn disk_twin(code: &PluginManifest) -> PluginManifest {
    let mut m = code.clone();
    m.version = "9.9.9".into();
    m
}

/// 运行期才存在的工具形态（`tool-mcp` 的远端发现就是这个形状）。
fn tool(name: &str, se: SideEffect) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: format!("remote tool {name}"),
        schema: serde_json::json!({ "type": "object" }),
        side_effect: se,
        exposure: Default::default(),
    }
}

fn filled(id: &str) -> PluginManifest {
    let mut m = base(id);
    official_meta(&mut m, ("回显", "Echo"), ("甲", "A"));
    m
}

#[test]
fn drift_is_silent_when_the_two_manifests_are_the_same_plugin() {
    let code = filled("tool-drift");
    assert_eq!(
        manifest_drift(&code, &disk_twin(&code)),
        Vec::<String>::new(),
        "只差 version 的同源两份必须零差异（这条不成立，下面几条报的红就不可信）"
    );
}

#[test]
fn drift_names_each_changed_field_instead_of_merging_them() {
    let code = filled("tool-a");
    let mut disk = code.clone();
    disk.id = "tool-b".into();
    disk.kind = PluginKind::Middleware;
    disk.abi_version = crate::PLUGIN_ABI_VERSION + 1;
    disk.tools[0].description = "换了个说法".into();
    disk.tools[0].schema = serde_json::json!({ "type": "string" });
    disk.tools[0].side_effect = SideEffect::Irreversible;
    disk.tools[0].exposure = Exposure::OnDemand;
    disk.display_name = Some(zh_en("别的", "Other"));
    disk.origin = PluginOrigin::Community;

    let d = manifest_drift(&code, &disk);
    for frag in [
        "id:",
        "kind:",
        "abi_version:",
        "`t`.description:",
        "`t`.schema:",
        "`t`.side_effect:",
        "`t`.exposure:",
        "display_name:",
        "origin:",
    ] {
        assert!(
            d.iter().any(|x| x.contains(frag)),
            "漏报 `{frag}`，实际清单: {d:?}"
        );
    }
    // 九处改动 = 九条，不许把两条并成一句含糊话（要能照抄改对）
    assert_eq!(d.len(), 9, "差异条数变了: {d:?}");
}

#[test]
fn code_may_only_append_tools_that_the_disk_list_never_saw() {
    let mut code = filled("tool-mcp-like");
    code.tools.push(tool("mcp_github_whoami", SideEffect::Pure));
    let mut disk = code.clone();
    disk.version = "9.9.9".into();
    disk.tools.truncate(1); // 清单只声明两个管理工具里的那一个静态位
    assert!(
        manifest_drift(&code, &disk).is_empty(),
        "「追加在后」是 tool-mcp 的真实形状，不该报: {:?}",
        manifest_drift(&code, &disk)
    );

    // 反例一：清单多报一个代码里根本没有的工具
    let mut ghost = disk.clone();
    ghost
        .tools
        .push(tool("never_implemented", SideEffect::Pure));
    let d = manifest_drift(&code, &ghost);
    assert_eq!(d.len(), 1, "该只报一条: {d:?}");
    assert!(d[0].contains("前缀"), "实际: {d:?}");

    // 反例二：把运行期才有的工具名塞进清单的第一位（替代而不是追加）——
    // 顺序在这里就是那条边界，不报就等于给"漏声明"开了后门
    let mut shuffled = disk.clone();
    shuffled.tools[0].name = "mcp_github_whoami".into();
    let d = manifest_drift(&code, &shuffled);
    assert!(d.iter().any(|x| x.contains("前缀")), "替代位没报: {d:?}");
}

#[test]
fn drift_against_file_reports_what_it_could_not_read() {
    let code = base("tool-file");
    // 目录在、`manifest.json` 不在 ⇒ 一条人话说清缺哪个文件，绝不 panic
    let d = drift_against_file(&code, concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
    assert_eq!(d.len(), 1, "实际: {d:?}");
    assert!(
        d[0].contains("读不到") && d[0].contains("manifest.json"),
        "实际: {d:?}"
    );
}

/// 插件侧那条测试的真实入口形状：`CString`（describe 的返回）→ 解析 → 比磁盘。
/// 这里指到本 crate 的 `src/`（没有 manifest.json）拿"读不到"这一条，证的是
/// **管道通**；每个插件那条测试负责证**同源**。
#[test]
fn drift_of_describe_takes_the_cstring_the_plugin_hands_over() {
    let d = drift_of_describe(
        &filled("tool-of-desc").to_describe_json(),
        concat!(env!("CARGO_MANIFEST_DIR"), "/src"),
    );
    assert_eq!(d.len(), 1, "实际: {d:?}");
    assert!(d[0].contains("读不到"), "实际: {d:?}");
    // 反面对照：坏 JSON 走差异而不是 panic（一声 panic 报出来的是栈，不是这句话）
    let broken = std::ffi::CString::new(b"{ not json".to_vec()).unwrap();
    let d = drift_of_describe(&broken, env!("CARGO_MANIFEST_DIR"));
    assert_eq!(d.len(), 1, "实际: {d:?}");
    assert!(d[0].contains("解析不回来"), "实际: {d:?}");
}
