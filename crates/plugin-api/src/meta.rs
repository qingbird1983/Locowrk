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
