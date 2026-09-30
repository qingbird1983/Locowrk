//! plugins_root 工件发现的单元测试（从 tests.rs 按职责拆出：平台
//! 工具函数自包含，不依赖内核 runner 桩件）。

use crate::plugins_root;

#[test]
fn dll_name_matches_platform() {
    let name = plugins_root::dll_name("tool-echo");
    if cfg!(windows) {
        assert_eq!(name, "tool_echo.dll");
    } else if cfg!(target_os = "macos") {
        assert_eq!(name, "libtool_echo.dylib");
    } else {
        assert_eq!(name, "libtool_echo.so");
    }
}

fn artifact_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "locowrk-find-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// packaging.md §3.1："dll 名字由作者决定，宿主不假设文件名"——约定名
/// 缺席时扫描目录里唯一 platform 动态库；多个则拒绝猜测。
#[test]
fn find_artifact_prefers_conventional_then_single_dynlib() {
    let dir = artifact_dir("artifact");

    // 约定名在位 → 直接用
    let conventional = dir.join(plugins_root::dll_name("tool-x"));
    std::fs::write(&conventional, b"x").unwrap();
    assert_eq!(
        plugins_root::find_artifact(&dir, "tool-x"),
        Some(conventional)
    );

    // 只有作者自定义名的唯一动态库 → 采用（manifest 为准的精神：不假设文件名）
    let custom = dir.join(format!(
        "my-weird-name.{}",
        if cfg!(windows) { "dll" } else { "so" }
    ));
    std::fs::remove_file(dir.join(plugins_root::dll_name("tool-x"))).unwrap();
    std::fs::write(&custom, b"x").unwrap();
    assert_eq!(
        plugins_root::find_artifact(&dir, "tool-x"),
        Some(custom.clone())
    );

    // 两个动态库 → 歧义拒绝
    std::fs::write(dir.join("another.dll"), b"x").unwrap();
    assert_eq!(plugins_root::find_artifact(&dir, "tool-x"), None);

    // 一个动态库 + 一个非库文件 → 仍用那个动态库
    std::fs::remove_file(dir.join("another.dll")).unwrap();
    std::fs::write(dir.join("manifest.json"), b"{}").unwrap();
    assert_eq!(plugins_root::find_artifact(&dir, "tool-x"), Some(custom));
}

/// §5.8（v2.16）：版本目录里也可以是 `.wasm` 工件——约定 wasm 名优先，
/// 自定义名扫描可达；native 约定名与 wasm 并存时 native 优先。
#[test]
fn find_artifact_accepts_wasm() {
    let dir = artifact_dir("wasm");

    // 约定 wasm 名
    let wasm = dir.join("tool-x.wasm");
    std::fs::write(&wasm, b"\0asm").unwrap();
    assert_eq!(
        plugins_root::find_artifact(&dir, "tool-x"),
        Some(wasm.clone())
    );

    // 自定义 wasm 名（唯一可装载工件）→ 采用
    let custom = dir.join("guest.wasm");
    std::fs::remove_file(&wasm).unwrap();
    std::fs::write(&custom, b"\0asm").unwrap();
    assert_eq!(
        plugins_root::find_artifact(&dir, "tool-x"),
        Some(custom.clone())
    );

    // native 与 wasm 并存：native 优先（约定名先查），无歧义不拒绝
    let native = dir.join(plugins_root::dll_name("tool-x"));
    std::fs::write(&native, b"x").unwrap();
    assert_eq!(plugins_root::find_artifact(&dir, "tool-x"), Some(native));
}

/// S2：开发态扫描尊重跳过集（lockfile 已接管 / 已停用的内置插件）。
/// 依赖真实工件（scripts/build_plugins.sh 暂存），缺失时 skip。
#[test]
fn load_all_honors_skip_set() {
    use std::collections::HashSet;
    use std::sync::Arc;

    let Some(root) = plugins_root::resolve() else {
        eprintln!("skip: plugins/ 未暂存（先跑 scripts/build_plugins.sh）");
        return;
    };
    let registry = Arc::new(plugin_host::PluginRegistry::new());
    let loaded = plugins_root::load_all(&root, &registry, &HashSet::new()).unwrap();
    assert!(!loaded.is_empty(), "无跳过集应能加载内置插件");

    // 全量跳过：一个都不装载
    let skip: HashSet<String> = loaded.iter().map(|(id, _)| id.clone()).collect();
    let registry2 = Arc::new(plugin_host::PluginRegistry::new());
    let loaded2 = plugins_root::load_all(&root, &registry2, &skip).unwrap();
    assert!(loaded2.is_empty(), "跳过集内的 id 不得装载（{loaded2:?}）");
    assert!(registry2.list_states().is_empty());
}
