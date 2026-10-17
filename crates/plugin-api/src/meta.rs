//! 官方插件的展示元数据基座（审计 F7 · 批 F-21c 上收）。
//!
//! 上收前实况（逐份读原文实测；门禁 `texts` 拿 `git show HEAD:` 的尾块实参与本函数
//! 实参三面回比，`uplift_meta` 数接线）：10 个内置插件的 `describe()` 尾部各自手写
//! 过同一段展示位——
//!
//! - `fn zh_en(zh, en) -> LocalizedString` **十份逐字节相同**（形态数 = 1；
//!   另有第 11 份埋在 `kernel-bin/src/scaffold.rs` 的模板串里，本批换成基座导入）
//! - `icon` / `author` / `origin` / `homepage` / `license` 五个字段**十份完全一致**
//! - 只有 `display_name` 与 `description` 的两条文案随插件不同（其中 tool-mcp 的
//!   description 是 `&format!(…)` 运行期拼的，所以基座收的是 `&str` 表达式而不是常量）
//!
//! 收益 = 10 份 × 约 17 行样板收成一处；"官方标识"这类安全默认（`origin` 缺省必须
//! 显式声明，见 `manifest.rs` §2.4）从十处手写变成一个出处。
//!
//! 不进 ABI 面（与 `args`/`fsutil`/`memlayout`/`netsafe` 同栏）：插件源码级依赖。
//! 依赖 `PluginManifest` 的 `impl Default`（`manifest.rs`）补齐字面量里不再手写的
//! 那七个可选字段。
use crate::manifest::{Author, LocalizedOrString, LocalizedString, PluginManifest, PluginOrigin};
use crate::serde_json::Value;
use crate::tool::ToolSpec;
use std::collections::BTreeMap;

/// 中英双语文案（键名固定 `zh-CN` / `en-US`，与 packaging.md §2.3 的回退链对齐）。
/// 十份原样上收，一条没改。
pub fn zh_en(zh: &str, en: &str) -> LocalizedString {
    LocalizedString(BTreeMap::from([
        ("zh-CN".to_owned(), zh.to_owned()),
        ("en-US".to_owned(), en.to_owned()),
    ]))
}

/// Locowrk 自带插件的展示位：填 `display_name` / `description` 两条文案，
/// 其余五个字段用十份共同的官方默认（图标相对路径、作者、`Official` 来源、
/// 无主页、MIT）。`origin` 必须显式声明为官方——缺省一律 community（防第三方包
/// 伪造"官方内置"徽章），所以这一句只此一处反而更安全。
///
/// 用法是「字面量留核心五字段 + `..Default::default()`，随后调用本函数」；
/// `PluginManifest` 的 Default 只补齐那七个可选元数据字段（`manifest.rs`），
/// 所以调用点少写的字段与手写尾块删掉的字段严格一一对应。
pub fn official_meta(m: &mut PluginManifest, name: (&str, &str), desc: (&str, &str)) {
    m.display_name = Some(zh_en(name.0, name.1));
    m.description = Some(LocalizedOrString::Localized(zh_en(desc.0, desc.1)));
    m.icon = Some("assets/icon.svg".into());
    m.author = Some(Author {
        name: "Locowrk Team".into(),
        url: None,
    });
    m.origin = PluginOrigin::Official;
    m.homepage = None;
    m.license = Some("MIT".into());
}

/// 唯一真源对账（台账 T11）：把插件**代码侧** describe 出来的 manifest 与同目录
/// `manifest.json` 逐字段比一遍，返回人类可读的差异清单（空清单 = 两份同源）。
///
/// **治的是什么**：这两个出处从来各写各的——UI 的名称/徽章读磁盘
/// （`plugin-service::display::read_display`），内核的工具池与副作用判定读代码
/// （`plugin-host::adapter` 合目录、`instance` 判 side_effect）。审计 H8 抓到过一次
/// "磁盘写 `Pure` 而代码早已 `Irreversible`"（见 `scripts/build_plugins.sh` 的 0.1.2
/// 变更注记）；2026-10-18 逐插件再比，又数出 15 段工具描述、6 处 schema 键、4 处
/// `exposure` 漂着——其中 `fs_write`/`fs_delete`/三个 `edit_*` 的
/// `expected_revision` 与 `memory_write` 的 `scope` 在磁盘清单里**根本不存在**，
/// 而它们是模型目录里那份并发写保护的说明文字。也就是说"以磁盘为准"会把这些抹掉，
/// "以代码为准"会让 UI 失去静态清单——先有个能当场报差异的东西，才谈得上定源。
///
/// **为什么两边都留着、靠判据钉相等而不是合成一份**：磁盘那份要能在插件**没加载**
/// 时也画出卡片（市场目录、禁用态），代码那份要能表达运行期才知道的事（`tool-mcp`
/// 的远端工具、schema 里指向 env 的提示）。任何"删掉一份"都会让另一半失去载体，
/// 所以这一刀做的是：**能力以代码为真、展示文案以两份必须相等为前提**，
/// 等值由每个插件的一条 `cargo test` 承重（插件 crate 都在根 workspace members 里，
/// 跑 `make check` 就跑得到），差异清单直接可读、照抄就能改对。
///
/// **不比 `version`**：它由 `scripts/build_plugins.sh` 的 PLAN 清单注入两侧
/// （代码侧 env 宏 + 产物 JSON 的 sed），源码里那份只是占位，比它只会制造假红。
///
/// **顺序也不比**：工具目录的顺序由代码决定（`merge_catalog` 之后还有排序），
/// 磁盘只是清单的书写顺序，比它会逼人去对齐两个本来无关的东西。
pub fn manifest_drift(code: &PluginManifest, disk: &PluginManifest) -> Vec<String> {
    let mut d = Vec::new();
    let json = |v: &Value| serde_json::to_string(v).unwrap_or_else(|_| format!("{v:?}"));
    if code.id != disk.id {
        d.push(format!("id: 代码=`{}` | 磁盘=`{}`", code.id, disk.id));
    }
    if code.kind != disk.kind {
        d.push(format!("kind: 代码={:?} | 磁盘={:?}", code.kind, disk.kind));
    }
    if code.abi_version != disk.abi_version {
        d.push(format!(
            "abi_version: 代码={} | 磁盘={}（宿主按磁盘这份校验会拒载）",
            code.abi_version, disk.abi_version
        ));
    }
    // 工具表的形状：**磁盘清单必须是代码清单的前缀**。
    // 为什么允许代码多出来：`tool-mcp` 在运行期从 `<数据根>/mcp.json` 发现远端工具
    // 并追加在两个管理工具之后（连 side_effect 都由用户配置的 grade 决定），
    // 这类工具静态清单天生承载不了。只准"追加在后"、不准"插在前面或替代"，
    // 是为了让"漏声明一个静态工具"照样报得出来——顺序在这里就是那条边界。
    let tool_names =
        |ts: &[ToolSpec]| -> Vec<String> { ts.iter().map(|t| t.name.clone()).collect() };
    if code.tools.len() < disk.tools.len() {
        d.push(format!(
            "磁盘清单声明 {} 个工具，代码只报出 {} 个（UI 会显示并不存在的工具）：代码={:?} | 磁盘={:?}",
            disk.tools.len(),
            code.tools.len(),
            tool_names(&code.tools),
            tool_names(&disk.tools)
        ));
    } else if disk
        .tools
        .iter()
        .zip(code.tools.iter())
        .any(|(a, b)| a.name != b.name)
    {
        d.push(format!(
            "磁盘清单不是代码清单的前缀（同位置工具名不同）：代码={:?} | 磁盘={:?}",
            tool_names(&code.tools),
            tool_names(&disk.tools)
        ));
    }
    for t in &code.tools {
        let Some(o) = disk.tools.iter().find(|o| o.name == t.name) else {
            continue;
        };
        if t.description != o.description {
            d.push(format!(
                "`{}`.description: 代码=`{}` | 磁盘=`{}`",
                t.name, t.description, o.description
            ));
        }
        if t.schema != o.schema {
            d.push(format!(
                "`{}`.schema: 代码={} | 磁盘={}",
                t.name,
                json(&t.schema),
                json(&o.schema)
            ));
        }
        if t.side_effect != o.side_effect {
            // 这条最贵：side_effect 决定要不要弹审批卡，两侧不同 = 用户看到的承诺
            // 与内核真实行为不一致（审计 H8 的同族）。
            d.push(format!(
                "`{}`.side_effect: 代码={:?} | 磁盘={:?}（审批行为按代码走，徽章按磁盘画）",
                t.name, t.side_effect, o.side_effect
            ));
        }
        if t.exposure != o.exposure {
            d.push(format!(
                "`{}`.exposure: 代码={:?} | 磁盘={:?}（少了 `always` 就是把它踢回候选池，可能被 Top-N 挤出目录）",
                t.name, t.exposure, o.exposure
            ));
        }
    }
    if code.display_name != disk.display_name {
        d.push(format!(
            "display_name: 代码={:?} | 磁盘={:?}",
            code.display_name, disk.display_name
        ));
    }
    if code.description != disk.description {
        d.push(format!(
            "description: 代码={:?} | 磁盘={:?}",
            code.description, disk.description
        ));
    }
    if code.icon != disk.icon {
        d.push(format!("icon: 代码={:?} | 磁盘={:?}", code.icon, disk.icon));
    }
    if code.author != disk.author {
        d.push(format!(
            "author: 代码={:?} | 磁盘={:?}",
            code.author, disk.author
        ));
    }
    if code.origin != disk.origin {
        d.push(format!(
            "origin: 代码={:?} | 磁盘={:?}（官方徽章的真源，不一致就是给第三方包发徽章）",
            code.origin, disk.origin
        ));
    }
    if code.homepage != disk.homepage {
        d.push(format!(
            "homepage: 代码={:?} | 磁盘={:?}",
            code.homepage, disk.homepage
        ));
    }
    if code.license != disk.license {
        d.push(format!(
            "license: 代码={:?} | 磁盘={:?}",
            code.license, disk.license
        ));
    }
    d
}

/// 同上，但磁盘那份**由本函数去读**：`dir` 传 `env!("CARGO_MANIFEST_DIR")`，
/// 取 `dir/manifest.json`。读不到或解析不了也写进差异清单而不是 panic——
/// "清单缺失/坏了"本身就是这条判据要报的事，而且一声 panic 报出来的是一堆栈
/// 而不是一句话。做成私有：插件侧只该看见 [`drift_of_describe`] 这一个入口。
fn drift_against_file(code: &PluginManifest, dir: &str) -> Vec<String> {
    let path = std::path::Path::new(dir).join("manifest.json");
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => return vec![format!("读不到 `{}`：{e}", path.display())],
    };
    match PluginManifest::from_json(&bytes) {
        Ok(disk) => manifest_drift(code, &disk),
        Err(e) => vec![format!("`{}` 解析失败：{e}", path.display())],
    }
}

/// 插件侧唯一的调用形状（台账 T11）：把 `describe_manifest()` 的返回值与同目录
/// `manifest.json` 比一遍，返回差异清单（空 = 同源）。插件里那条测试只有三行：
///
/// ```ignore
/// #[test]
/// fn manifest_json_and_describe_agree() {
///     let d = plugin_api::meta::drift_of_describe(&describe_manifest(), env!("CARGO_MANIFEST_DIR"));
///     assert!(d.is_empty(), "磁盘清单与代码 describe 漂了：\n{}", d.join("\n"));
/// }
/// ```
///
/// 收成这一个入口的理由与 [`official_meta`] 完全同一条：让十个插件各写一遍
/// "怎么读磁盘、怎么解析回来、比哪些字段"，第九个一定漏比某个字段——而漏掉的那
/// 个字段正是漂移藏身的地方（本轮实测漂在 `expected_revision` 与 `exposure` 上）。
pub fn drift_of_describe(described: &std::ffi::CString, dir: &str) -> Vec<String> {
    let code = match PluginManifest::from_json(described.as_bytes()) {
        Ok(m) => m,
        // 这条不是理论风险：`to_describe_json` 之后又手改字段、或 serde 形态变了，
        // 都会让"宿主实际读到的那份"与"我们对账的那份"不是同一个东西。
        Err(e) => return vec![format!("describe 自己产出的 JSON 解析不回来：{e}")],
    };
    drift_against_file(&code, dir)
}

/// 把磁盘清单**对齐到代码那份**（维护动作，不是判据的一部分）：展示七字段与
/// `tools` 段整段取代码侧，`id` / `version` / `abi_version` / `kind` 保留磁盘原值
/// ——`version` 是构建脚本注入的占位，被代码那份（dev 态 = `0.0.0-dev`）覆盖就等于
/// 把打包版本号洗掉。
///
/// **只由插件测试在 `LOCOWRK_MANIFEST_SYNC=1` 时调用**，默认路径永远是"只报不改"：
/// 判据不许顺手改仓库文件（这条改的是**受版本控制**的清单，改完必须人看 diff 再提交）。
/// 存在的理由很实际：代码里一段工具描述是三五行的 Rust 字符串，手工往 JSON 里搬
/// 十个工具 × 十个插件必然搬错，而搬错的后果正是这条判据要防的那件事。
///
/// 输出按固定键序书写（`id`→`version`→`abi_version`→`kind`→展示→`tools`），
/// 缩进两格——它是**人手要读的文件**，用 `to_string_pretty` 会按字母序重排十个插件
/// 的清单，diff 一片糊，等于把可读性换成了一次自动化。
pub fn sync_manifest_file(described: &std::ffi::CString, dir: &str) -> Result<String, String> {
    let code = PluginManifest::from_json(described.as_bytes()).map_err(|e| e.to_string())?;
    let path = std::path::Path::new(dir).join("manifest.json");
    let before = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let disk: serde_json::Value =
        serde_json::from_slice(&before).map_err(|e| format!("{}: {e}", path.display()))?;
    let keep = |k: &str| -> String {
        disk.get(k)
            .map(|v| v.to_string())
            .unwrap_or_else(|| "null".to_string())
    };
    // pretty 打印的**续行**要整体再缩两格，否则 `"tools": [` 里面的层级会跟外面错位
    let block = |v: &serde_json::Value| -> String {
        let s = serde_json::to_string_pretty(v).unwrap_or_else(|_| v.to_string());
        s.split('\n')
            .enumerate()
            .map(|(i, l)| {
                if i == 0 {
                    l.to_string()
                } else {
                    format!("  {l}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let put = |out: &mut Vec<String>, k: &str, v: Option<serde_json::Value>| {
        if let Some(v) = v {
            out.push(format!("  \"{k}\": {}", block(&v)));
        }
    };
    let described_value = serde_json::to_value(&code).map_err(|e| e.to_string())?;
    // `tools` 只回写**静态前缀**（条数取磁盘清单原有条数）：磁盘清单的语义是"不装载也
    // 能声明的那部分"，而 `tool-mcp` 的 describe 会在两个管理工具之后追加运行期发现的
    // 远端工具——那些名字来自用户当时的 `mcp.json`，写进清单就等于把一个每次装载都变的
    // 快照钉死成静态声明（下一次装载远端一变，对账立刻红在那条"前缀"规则上）。
    // 于是本函数的边界是：**加静态工具要先手工把清单条数加上，再跑一次同步**。
    let disk_n = disk
        .get("tools")
        .and_then(Value::as_array)
        .map_or(0, |a| a.len());
    let static_tools: Vec<&ToolSpec> = code.tools.iter().take(disk_n).collect();
    let mut out = vec![
        format!("  \"id\": {}", keep("id")),
        format!("  \"version\": {}", keep("version")),
        format!("  \"abi_version\": {}", keep("abi_version")),
        format!("  \"kind\": {}", keep("kind")),
    ];
    for k in [
        "display_name",
        "description",
        "icon",
        "author",
        "origin",
        "homepage",
        "license",
    ] {
        put(&mut out, k, described_value.get(k).cloned());
    }
    put(
        &mut out,
        "tools",
        Some(serde_json::to_value(&static_tools).unwrap()),
    );
    let text = format!("{{\n{}\n}}\n", out.join(",\n"));
    std::fs::write(&path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(format!(
        "{}（静态 {} 条；代码另报 {} 条，差额=运行期追加）",
        path.display(),
        disk_n,
        code.tools.len()
    ))
}

/// 插件自维的版本号：读编译期环境变量 `LOCOWRK_PLUGIN_VERSION`（由各插件的
/// `build.rs` 声明 `rerun-if-env-changed`，`scripts/build_plugins.sh` 按 stage
/// 清单逐版注入），没给环境变量时回落 `"0.0.0-dev"`。
///
/// **必须是宏而不是函数**：`option_env!` 在**书写处**展开——写进基座函数里就会
/// 读成 plugin-api 自己的编译环境（一次编译、十个插件共用），版本号会集体变成
/// 同一个值甚至全变 `0.0.0-dev`。宏在调用它的插件 crate 里展开 ⇒ 语义与上收前
/// 十份 `fn plugin_version()` 逐字相同（等值证明 = 门禁 `version_equiv` 拿
/// `git show HEAD:` 的十份原文与本宏体做归一化比较）。
///
/// 收成一处真正消灭的缺陷面：环境变量名与回落字面量此前有十份拷贝，任何一份
/// 拼错都会**静默**变成 `0.0.0-dev`（`option_env!` 拼错的名字照样编译通过）。
#[macro_export]
macro_rules! plugin_version {
    () => {
        option_env!("LOCOWRK_PLUGIN_VERSION").unwrap_or("0.0.0-dev")
    };
}

#[cfg(test)]
mod tests;
