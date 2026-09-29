//! 脚手架测试：目录树 / manifest 可解析 / 名称清洗 / 已存在拒绝 / 依赖路径探测。

use std::path::PathBuf;

use super::{new_plugin, plugin_api_dep, sanitize_id, struct_name};
use plugin_api::manifest::PluginManifest;
use plugin_api::tool::SideEffect;

fn temp_root(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "locowrk-scaffold-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 生成的目录树与 packaging.md §8 一致，manifest 按 v2 解析。
#[test]
fn scaffold_generates_doc_tree_with_parseable_manifest() {
    let root = temp_root("gen");
    let dir = new_plugin("my-plugin", &root).unwrap();
    for f in [
        "Cargo.toml",
        "manifest.json",
        "src/lib.rs",
        "assets/icon.svg",
    ] {
        assert!(dir.join(f).is_file(), "缺少 {f}");
    }

    let bytes = std::fs::read(dir.join("manifest.json")).unwrap();
    let m = PluginManifest::from_json(&bytes).expect("脚手架 manifest 必须可解析");
    assert_eq!(m.id, "my-plugin");
    assert_eq!(m.version, "0.1.0");
    assert_eq!(m.tools.len(), 1);
    assert_eq!(m.tools[0].name, "my_plugin");
    // 工具级双语描述压平：pick("en-US")（④a 宽容解析行为）
    assert_eq!(m.tools[0].description, "Tool description");
    assert_eq!(m.tools[0].side_effect, SideEffect::Pure);
    // origin 不写 → 缺省 community（§2.4 安全默认）
    assert_eq!(
        m.origin,
        plugin_api::manifest::PluginOrigin::Community,
        "缺省一律 community，官方标识必须显式声明"
    );
    assert_eq!(m.icon.as_deref(), Some("assets/icon.svg"));
    assert!(
        std::fs::read(dir.join("assets/icon.svg"))
            .unwrap()
            .starts_with(b"<svg"),
        "占位图标就位"
    );

    let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    assert!(cargo.contains("crate-type = [\"cdylib\"]"), "产物是动态库");
    assert!(cargo.contains("plugin-api"), "只依赖冻结契约层");
    let lib = std::fs::read_to_string(dir.join("src/lib.rs")).unwrap();
    assert!(lib.contains("declare_plugin!"), "经声明宏导出 C ABI");
}

/// 名称清洗（§2.2）与拒绝路径：已存在不覆盖、清洗后为空拒绝。
#[test]
fn scaffold_sanitizes_names_and_rejects_existing_or_empty() {
    let root = temp_root("sanitize");
    let dir = new_plugin("My_Plugin 2", &root).unwrap();
    assert_eq!(dir.file_name().unwrap().to_str().unwrap(), "my-plugin-2");

    assert!(
        new_plugin("my-plugin-2", &root).is_err(),
        "已存在 → 拒绝覆盖"
    );
    assert!(new_plugin("!!!", &root).is_err(), "全非法字符 → 拒绝");
    assert!(sanitize_id("!!!中文").is_none(), "中文不算合法 id 字符");
    assert_eq!(sanitize_id("-a-"), Some("a".into()), "首尾连字符剥掉");
    assert_eq!(struct_name("my-plugin"), "MyPlugin");
}

/// 依赖路径探测：仓库内生成 → 相对路径；仓库外 → 占位符。
#[test]
fn scaffold_detects_plugin_api_path_by_walking_up() {
    let repo = temp_root("fake-repo");
    std::fs::create_dir_all(repo.join("crates/plugin-api")).unwrap();
    std::fs::write(repo.join("crates/plugin-api/Cargo.toml"), "[package]").unwrap();
    let dir = new_plugin("walk-up", &repo.join("tmp").join("x")).unwrap();
    let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    assert!(
        cargo.contains("../../../crates/plugin-api"),
        "三级目录向上应定位到仓库内 plugin-api：{cargo}"
    );

    let outside = temp_root("outside");
    let dir = new_plugin("outside-plugin", &outside).unwrap();
    let cargo = std::fs::read_to_string(dir.join("Cargo.toml")).unwrap();
    assert!(
        cargo.contains("REPLACE_WITH_LOCOWRK_PATH"),
        "仓库外写占位符：{cargo}"
    );
    assert_eq!(
        plugin_api_dep(&outside.join("nowhere")),
        "REPLACE_WITH_LOCOWRK_PATH/crates/plugin-api"
    );
}
