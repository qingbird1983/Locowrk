//! 内置插件停用/登记（S2，清单 §1）的编排层测试。涉及真实加载的用例
//! 依赖 `scripts/build_market.sh` 暂存的 tool-shout 工件，缺失时 skip
//! （mod.rs 头部注释的同款约定）。

use super::*;

/// 造一个假内置根：拷贝暂存的 tool-shout 版本目录（manifest + 工件）。
fn fake_builtin_root(root: &Path, vers: &[Version]) {
    fake_market_from_staged(root, vers);
}

#[tokio::test]
async fn builtin_register_skip_and_reopen_keeps_disabled() {
    let data = temp_dir("breg");
    let market_root = temp_dir("breg-market");
    let builtin_root = temp_dir("breg-builtin");
    let svc = service_with_builtin(
        data.clone(),
        market_root.clone(),
        Some(builtin_root.clone()),
    );

    // 首次启动登记：条目 Builtin + enabled，不在扫描跳过集
    svc.register_builtin("tool-x", &Version::new(1, 0, 0));
    let lf = LockFile::load(&data.join("lockfile.json")).unwrap();
    let e = lf.entry("tool-x").unwrap();
    assert_eq!(e.source, crate::lockfile::InstallSource::Builtin);
    assert!(e.enabled);
    assert!(!svc.dev_scan_skip_ids().contains(&"tool-x".to_string()));

    // 停用：落盘 enabled:false，进入扫描跳过集
    svc.set_enabled("tool-x", false).await.unwrap();
    assert!(
        svc.dev_scan_skip_ids().contains(&"tool-x".to_string()),
        "停用的内置插件必须被启动扫描跳过"
    );

    // 重启（新 service 同数据根）：登记再次发生也不复活停用状态
    let svc2 = service_with_builtin(data.clone(), market_root, Some(builtin_root));
    svc2.register_builtin("tool-x", &Version::new(1, 0, 0));
    let back = LockFile::load(&data.join("lockfile.json")).unwrap();
    assert!(!back.entry("tool-x").unwrap().enabled, "重启不得复活停用项");

    // 版本独立升级：登记新版本只同步版本号，enabled 保持
    svc2.register_builtin("tool-x", &Version::new(1, 1, 0));
    let back = LockFile::load(&data.join("lockfile.json")).unwrap();
    let e = back.entry("tool-x").unwrap();
    assert_eq!(e.version, Version::new(1, 1, 0));
    assert!(!e.enabled);
}

#[tokio::test]
async fn builtin_uninstall_and_update_are_refused() {
    let data = temp_dir("bguard");
    let svc = service_with_builtin(data, temp_dir("bguard-market"), Some(temp_dir("bguard-b")));
    svc.register_builtin("tool-x", &Version::new(1, 0, 0));

    let err = svc.uninstall("tool-x").await.unwrap_err();
    assert!(err.contains("built in"), "{err}");
    let err = svc.update("tool-x").await.unwrap_err();
    assert!(err.contains("built in"), "{err}");
    assert!(
        LockFile::load(&svc.lockfile_path())
            .unwrap()
            .entry("tool-x")
            .is_some()
    );
}

#[tokio::test]
async fn reconcile_keeps_builtin_entry_without_install_dir() {
    // 内置插件不在安装根（开发态目录才是家）：对账不得当"目录缺失"清除
    let data = temp_dir("brec");
    let svc = service_with_builtin(
        data.clone(),
        temp_dir("brec-market"),
        Some(temp_dir("brec-b")),
    );
    svc.register_builtin("tool-x", &Version::new(1, 0, 0));
    svc.startup_reconcile().unwrap();
    let back = LockFile::load(&data.join("lockfile.json")).unwrap();
    assert!(back.entry("tool-x").is_some(), "内置条目不应对账清除");
}

/// 启用内置插件：从内置根解析**最新**版本（独立升级，登记版本可能滞后），
/// 版本漂移同步回条目；停用后 active 摘除。
#[tokio::test]
async fn builtin_enable_resolves_latest_and_syncs_version() {
    let vers = staged_versions();
    if vers.is_empty() {
        eprintln!("skip: market/tool-shout 未暂存（先跑 scripts/build_market.sh）");
        return;
    }
    let data = temp_dir("benable");
    let builtin_root = temp_dir("benable-builtin");
    fake_builtin_root(&builtin_root, &vers);
    let svc = service_with_builtin(data, temp_dir("benable-market"), Some(builtin_root));

    let oldest = &vers[0];
    let newest = &vers[vers.len() - 1];
    svc.register_builtin("tool-shout", oldest);

    svc.set_enabled("tool-shout", true).await.unwrap();
    let listed = svc.list_installed();
    assert_eq!(
        listed[0].active_version,
        Some(newest.to_string()),
        "启用应取内置根最新版本"
    );
    let lf = LockFile::load(&svc.lockfile_path()).unwrap();
    assert_eq!(
        lf.entry("tool-shout").unwrap().version,
        *newest,
        "版本漂移应同步回条目"
    );

    svc.set_enabled("tool-shout", false).await.unwrap();
    assert_eq!(svc.list_installed()[0].active_version, None);
    assert!(svc.dev_scan_skip_ids().contains(&"tool-shout".to_string()));
}

/// 市场安装同 id：条目转 Market（安装态接管），开发态扫描让位。
#[tokio::test]
async fn market_install_takes_over_builtin_entry() {
    let vers = staged_versions();
    if vers.is_empty() {
        eprintln!("skip: market/tool-shout 未暂存（先跑 scripts/build_market.sh）");
        return;
    }
    let data = temp_dir("btakeover");
    let market_root = temp_dir("btakeover-market");
    fake_market_from_staged(&market_root, &vers);
    let svc = service_with_builtin(data, market_root.clone(), Some(temp_dir("btakeover-b")));

    svc.register_builtin("tool-shout", &vers[0]);
    svc.install("tool-shout", Some(&vers[0].to_string()))
        .await
        .unwrap();

    let lf = LockFile::load(&svc.lockfile_path()).unwrap();
    assert_eq!(
        lf.entry("tool-shout").unwrap().source,
        crate::lockfile::InstallSource::Market,
        "市场安装接管后条目应为 Market 来源"
    );
    assert!(
        svc.dev_scan_skip_ids().contains(&"tool-shout".to_string()),
        "安装态接管后开发态扫描必须让位（防同 id 双版本互踩）"
    );
}
