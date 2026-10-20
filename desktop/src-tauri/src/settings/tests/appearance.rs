//! 外观区设置测试（⑰；UI 设计 v3.3 起缺省 light）：缺省 light / 旧文件
//! 兼容（无 appearance 区也随缺省变亮色，§8.3-1 裁定）/ 更新往返 / 非法值拒绝。

use super::*;
// 值域三常量按全路径取，不走 settings.rs 的 `pub use`：那两个 `THEME_*` 也没抬，
// 而 settings.rs:14–17 那条注释记的就是「re-export 在非 test 目标里没人读 =
// unused imports，门禁 `-D warnings` 当场红」。本簇要的只是测试里不手抄数字，
// 路径本身对 settings 的后代模块可见（appearance 是它的私有子模块）。
use crate::settings::appearance::{UI_FONT_DEFAULT, UI_FONT_MAX, UI_FONT_MIN};

#[test]
fn appearance_defaults_legacy_compat_and_update() {
    let root = temp_root("appearance");
    let store = SettingsStore::open(&root).unwrap();
    // 缺省 light——用户 2026-10-01 裁定；旧 settings.json 无 appearance 区
    // 时也随缺省变亮色（属预期），显式存过 dark 的文件不受影响
    assert_eq!(store.view().appearance.theme, "light");
    std::fs::write(root.join("settings.json"), r#"{"locale": "zh-CN"}"#).unwrap();
    let v = SettingsStore::open(&root).unwrap().view().appearance;
    assert_eq!(v.theme, "light");
    std::fs::write(
        root.join("settings.json"),
        r#"{"locale": "zh-CN", "appearance": {"theme": "dark"}}"#,
    )
    .unwrap();
    let v = SettingsStore::open(&root).unwrap().view().appearance;
    assert_eq!(v.theme, "dark");
    // 更新 light + 磁盘往返
    store
        .update(
            None,
            Some(AppearanceUpdate {
                theme: Some("light".into()),
                ui_font_size: None,
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    let v = SettingsStore::open(&root).unwrap().view().appearance;
    assert_eq!(v.theme, "light");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn rejects_unknown_theme() {
    let root = temp_root("appearance-invalid");
    let store = SettingsStore::open(&root).unwrap();
    assert!(store
        .update(
            None,
            Some(AppearanceUpdate {
                theme: Some("sepia".into()),
                ui_font_size: None,
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .is_err());
    // 拒绝后内存态不被污染（仍为缺省 light）
    assert_eq!(store.view().appearance.theme, "light");
    std::fs::remove_dir_all(&root).ok();
}

/// 字号基座的缺省等价性（队列 Q14 第四刀的核心一条）。
///
/// 14px 原本是 `style.css` 里写死的 `--lw-ui-font-size`，本刀把它做成可调项。
/// 「可调项缺省必须等于改前的写死值」这条口径在这里取证：三种历史文件形态
/// （空文件 / 无 appearance 区 / 有 appearance 区但**没有** uiFontSize 键）都必须
/// 读出 14，且第三种里 theme 仍要保住——`#[serde(default)]` 是**容器级**语义
/// （缺失字段取 `Self::default()` 的对应值），写成字段级就会把整区覆盖成缺省、
/// 老用户的 dark 主题被冲掉。这条断言盯的就是这个区分。
#[test]
fn ui_font_size_defaults_to_the_value_css_used_to_hardcode() {
    let root = temp_root("ui_font_default");
    // 三行**字面锚**：断言常量自己等于那三个数。看着像同义反复，其实是唯一能抓住
    // "把缺省顺手改成 16"的写法——本簇其余断言全拿常量比常量（`== UI_FONT_DEFAULT`），
    // 常量一漂就一起跟着漂、整簇仍 5 passed。M1 变异第一跑就是这么打空的，
    // 这一档必须由字面数承重：14 = 本可调项出现前 `style.css` 里写死的那颗，
    // 「可调项缺省 = 改前写死值」那条口径在这里、也只在 `tmp/q14d-shape.mjs` A4 有对账。
    assert_eq!(
        UI_FONT_DEFAULT, 14,
        "缺省必须等于改前 style.css 写死的那颗 14px"
    );
    assert_eq!(UI_FONT_MIN, 12, "下限 = xs 档（基座−4）不越过 8px 可读地板");
    assert_eq!(
        UI_FONT_MAX, 18,
        "上限 = xl 档（基座+4）不越过 22px 正文上限"
    );
    let store = SettingsStore::open(&root).unwrap();
    assert_eq!(store.view().appearance.ui_font_size, UI_FONT_DEFAULT);
    for legacy in [
        r#"{"locale": "zh-CN"}"#,
        r#"{"locale": "zh-CN", "appearance": {"theme": "dark"}}"#,
    ] {
        std::fs::write(root.join("settings.json"), legacy).unwrap();
        let v = SettingsStore::open(&root).unwrap().view().appearance;
        assert_eq!(v.ui_font_size, UI_FONT_DEFAULT, "{legacy} 读出的基座");
    }
    // 上面那个循环跑完，最后写入的是 dark 那份：主题不能被新字段冲掉
    assert_eq!(
        SettingsStore::open(&root).unwrap().view().appearance.theme,
        "dark",
        "旧 appearance 区里存过的 theme 仍在"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// 往返 + 序列化形状 + 部分更新不互相踩踏。
/// 磁盘键名断言是前端 `types/settings.ts` 的镜像凭证：Rust 侧
/// `#[serde(rename_all = "camelCase")]` 输出 `uiFontSize`，前端 interface 写错
/// 一个字母则类型检查全绿而运行时 undefined（历批夹具照想象编的那条教训）。
#[test]
fn ui_font_size_round_trips_and_theme_only_update_keeps_it() {
    let root = temp_root("ui_font_roundtrip");
    let store = SettingsStore::open(&root).unwrap();
    store
        .update(
            None,
            Some(AppearanceUpdate {
                theme: None,
                ui_font_size: Some(16),
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    assert_eq!(store.view().appearance.ui_font_size, 16);
    let disk = std::fs::read_to_string(root.join("settings.json")).unwrap();
    assert!(
        disk.contains(r#""uiFontSize": 16"#),
        "磁盘键名必须是 camelCase 的 uiFontSize；实际 appearance 区：{:?}",
        disk.split_once("\"appearance\"")
            .map(|x| x.1)
            .unwrap_or_default()
    );
    // 只发 theme 的一路不得把字号顶回缺省
    store
        .update(
            None,
            Some(AppearanceUpdate {
                theme: Some("dark".into()),
                ui_font_size: None,
            }),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
        .unwrap();
    let v = SettingsStore::open(&root).unwrap().view().appearance;
    assert_eq!(v.theme, "dark");
    assert_eq!(v.ui_font_size, 16, "部分更新：缺席字段保持不变");
    std::fs::remove_dir_all(&root).ok();
}

/// 值域闭合：两端点各含（防「区间写成开区间」这种一字符错，以及「把缺省 14
/// 误当上限」），端点外一律拒且不污染内存态。
#[test]
fn ui_font_size_range_is_inclusive_and_rejects_outside() {
    let root = temp_root("ui_font_range");
    let store = SettingsStore::open(&root).unwrap();
    for px in [UI_FONT_MIN, UI_FONT_DEFAULT, UI_FONT_MAX] {
        let v = store
            .update(
                None,
                Some(AppearanceUpdate {
                    theme: None,
                    ui_font_size: Some(px),
                }),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .expect("端点内必须收");
        assert_eq!(v.appearance.ui_font_size, px);
    }
    for px in [UI_FONT_MIN - 1, UI_FONT_MAX + 1, 0, 99] {
        assert!(
            store
                .update(
                    None,
                    Some(AppearanceUpdate {
                        theme: None,
                        ui_font_size: Some(px),
                    }),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                )
                .is_err(),
            "{px} 越界必须拒而不是钳到边界"
        );
        // 拒绝后仍是上一次成功写入的值（最后一次成功 = UI_FONT_MAX）
        assert_eq!(store.view().appearance.ui_font_size, UI_FONT_MAX);
    }
    std::fs::remove_dir_all(&root).ok();
}
