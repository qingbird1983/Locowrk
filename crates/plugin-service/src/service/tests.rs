//! 编排层测试。需要真实 DLL 的用例依赖 `scripts/build_market.sh` 暂存的
//! `market/tool-shout/{ver}/`（工作区根），缺失时打印 skip 并通过——
//! 保证"裸 cargo test"与"完整验收"两种环境都可运行。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cordis_core::Context;
use plugin_host::PluginRegistry;
use semver::Version;

use super::PluginService;
use crate::lockfile::LockFile;
use crate::market::{LocalDirMarket, PackageManifest};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
}

/// 脚本暂存的市场版本（升序）。
fn staged_versions() -> Vec<Version> {
    let dir = repo_root().join("market").join("tool-shout");
    let mut vers: Vec<Version> = std::fs::read_dir(&dir)
        .map(|entries| {
            entries
                .flatten()
                .filter_map(|e| {
                    e.path()
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(String::from)
                })
                .filter_map(|name| Version::parse(&name).ok())
                .collect()
        })
        .unwrap_or_default();
    vers.sort();
    vers
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "locowrk-service-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// 把暂存版本拷进临时假市场（隔离真实 market/ 与 ~/.agent-kernel）。
fn fake_market_from_staged(root: &Path, vers: &[Version]) {
    let src_base = repo_root().join("market").join("tool-shout");
    for v in vers {
        let from = src_base.join(v.to_string());
        let to = root.join("tool-shout").join(v.to_string());
        std::fs::create_dir_all(&to).unwrap();
        for f in [
            "manifest.json".to_string(),
            crate::artifact_file_name("tool-shout"),
        ] {
            std::fs::copy(from.join(&f), to.join(&f)).unwrap();
        }
    }
}

fn service_in(data_dir: PathBuf, market_root: PathBuf) -> Arc<PluginService> {
    let registry = Arc::new(PluginRegistry::new());
    let ctx = Arc::new(Context::new());
    Arc::new(
        PluginService::open(
            data_dir,
            registry,
            ctx,
            Box::new(LocalDirMarket::new(market_root)),
        )
        .unwrap(),
    )
}

fn write_pkg_manifest(market_root: &Path, id: &str, v: &Version, description: &str) {
    let dir = market_root.join(id).join(v.to_string());
    std::fs::create_dir_all(&dir).unwrap();
    let m = PackageManifest {
        id: id.to_string(),
        version: v.to_string(),
        description: description.to_string(),
    };
    std::fs::write(
        dir.join("manifest.json"),
        serde_json::to_string_pretty(&m).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
async fn reconcile_cleans_tombstones_and_drops_missing_dirs() {
    let data = temp_dir("recon");
    let market_root = temp_dir("recon-market");

    // 预置 lockfile：一条 tombstone（可删目录）+ 一条目录缺失 + 一条禁用的
    let locked_dir = data.join("plugins").join("gone");
    std::fs::create_dir_all(&locked_dir).unwrap();
    let missing = data.join("plugins").join("tool-phantom").join("1.0.0");
    let present = data.join("plugins").join("tool-disabled").join("0.1.0");
    std::fs::create_dir_all(&present).unwrap();

    let mut lf = LockFile {
        schema_version: crate::lockfile::LOCKFILE_SCHEMA_VERSION,
        ..LockFile::default()
    };
    lf.pending_delete.push(locked_dir.clone());
    lf.add(
        "tool-phantom",
        Version::new(1, 0, 0),
        1,
        crate::lockfile::InstallSource::Market,
    );
    lf.add(
        "tool-disabled",
        Version::new(0, 1, 0),
        2,
        crate::lockfile::InstallSource::Market,
    );
    lf.set_enabled("tool-disabled", false).unwrap();
    lf.save(&data.join("lockfile.json")).unwrap();
    let _ = missing; // 故意不创建

    let svc = service_in(data.clone(), market_root);
    svc.startup_reconcile().unwrap();

    let back = LockFile::load(&data.join("lockfile.json")).unwrap();
    assert!(!locked_dir.exists(), "tombstone 应被清理");
    assert!(back.pending_delete.is_empty());
    assert!(
        back.entry("tool-phantom").is_none(),
        "目录缺失的条目应对账清除"
    );
    assert!(back.entry("tool-disabled").is_some(), "禁用条目保留");
    // 禁用插件不该被激活
    assert!(svc.list_installed()[0].active_version.is_none());
}

#[tokio::test]
async fn full_lifecycle_with_staged_artifact() {
    let vers = staged_versions();
    if vers.is_empty() {
        eprintln!("skip: market/tool-shout 未暂存（先跑 scripts/build_market.sh）");
        return;
    }
    let base = &vers[0];
    let data = temp_dir("flow");
    let market_root = temp_dir("flow-market");
    fake_market_from_staged(&market_root, std::slice::from_ref(base));
    // 目录里已有哪些版本就写哪些 manifest（staged 的 manifest 一并拷贝，
    // 这里无需重写）

    let svc = service_in(data.clone(), market_root.clone());
    let mut rx = svc.subscribe();

    // 安装 → active + lockfile 登记
    let got = svc
        .install("tool-shout", Some(&base.to_string()))
        .await
        .unwrap();
    assert_eq!(got, *base);
    let listed = svc.list_installed();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].active_version, Some(base.to_string()));
    assert!(listed[0].enabled);

    // 禁用 → active 摘除；启用 → 恢复
    svc.set_enabled("tool-shout", false).await.unwrap();
    assert_eq!(svc.list_installed()[0].active_version, None);
    svc.set_enabled("tool-shout", true).await.unwrap();
    assert_eq!(
        svc.list_installed()[0].active_version,
        Some(base.to_string())
    );

    // 卸载：条目消失；Windows 上已加载 DLL 被锁 → 目录走 tombstone
    svc.uninstall("tool-shout").await.unwrap();
    let back = LockFile::load(&data.join("lockfile.json")).unwrap();
    assert!(back.entry("tool-shout").is_none());
    let dir = data.join("plugins").join("tool-shout");
    assert!(
        !dir.exists() || back.pending_delete.contains(&dir),
        "目录应被删除或登记 tombstone（Windows 文件锁）"
    );
    let mut seen = Vec::new();
    while let Ok(ev) = rx.try_recv() {
        seen.push(ev);
    }
    assert_eq!(
        seen.len(),
        4,
        "installed/禁用/启用/uninstalled 各一条，实际 {seen:?}"
    );
    assert!(matches!(
        seen[0],
        super::PluginStateChange::Installed { .. }
    ));
    assert!(matches!(
        seen[1],
        super::PluginStateChange::EnabledChanged { enabled: false, .. }
    ));
    assert!(matches!(
        seen[2],
        super::PluginStateChange::EnabledChanged { enabled: true, .. }
    ));
    assert!(matches!(
        seen[3],
        super::PluginStateChange::Uninstalled { .. }
    ));
}

#[tokio::test]
async fn update_failure_keeps_old_active_and_cleans_new_dir() {
    let vers = staged_versions();
    if vers.is_empty() {
        eprintln!("skip: market/tool-shout 未暂存");
        return;
    }
    let base = &vers[0];
    let data = temp_dir("updfail");
    let market_root = temp_dir("updfail-market");
    fake_market_from_staged(&market_root, std::slice::from_ref(base));

    let svc = service_in(data.clone(), market_root.clone());
    svc.install("tool-shout", Some(&base.to_string()))
        .await
        .unwrap();

    // 造一个"更新版本"：manifest 合法但 DLL 是垃圾字节 → load 失败 → 回滚
    let bad = Version::new(base.major, base.minor, base.patch + 1);
    write_pkg_manifest(&market_root, "tool-shout", &bad, "broken update");
    std::fs::write(
        market_root
            .join("tool-shout")
            .join(bad.to_string())
            .join(crate::artifact_file_name("tool-shout")),
        b"not a dll",
    )
    .unwrap();

    let err = svc.update("tool-shout").await.unwrap_err();
    assert!(err.contains("rolled back"), "实际错误：{err}");
    assert_eq!(
        svc.list_installed()[0].active_version,
        Some(base.to_string()),
        "旧 active 指针应保持"
    );
    let bad_dir = data
        .join("plugins")
        .join("tool-shout")
        .join(bad.to_string());
    assert!(
        !bad_dir.exists(),
        "加载即失败的新目录应被清理（未映射，可删）"
    );
}

#[tokio::test]
async fn update_then_rollback_switches_active_version() {
    let vers = staged_versions();
    if vers.len() < 2 {
        eprintln!("skip: 需要两个暂存版本才能验证更新+回退");
        return;
    }
    let (old, new) = (vers[0].clone(), vers[vers.len() - 1].clone());
    let data = temp_dir("updown");
    let market_root = temp_dir("updown-market");
    fake_market_from_staged(&market_root, &vers);

    let svc = service_in(data.clone(), market_root);
    svc.install("tool-shout", Some(&old.to_string()))
        .await
        .unwrap();
    assert_eq!(svc.list_installed()[0].version, old.to_string());

    // 更新到 latest：active 切换，旧版本目录保留
    let got = svc.update("tool-shout").await.unwrap();
    assert_eq!(got, new);
    let listed = svc.list_installed();
    assert_eq!(listed[0].version, new.to_string());
    assert_eq!(listed[0].active_version, Some(new.to_string()));
    assert!(
        data.join("plugins")
            .join("tool-shout")
            .join(old.to_string())
            .is_dir(),
        "旧版本目录必须保留（多版本并存）"
    );

    // 回退：无需下载，只切指针
    svc.rollback("tool-shout", &old.to_string()).await.unwrap();
    let listed = svc.list_installed();
    assert_eq!(listed[0].version, old.to_string());
    assert_eq!(listed[0].active_version, Some(old.to_string()));
    assert!(listed[0].loaded_versions.contains(&new.to_string()));

    // 没有更新版本的 update 报"无新版"
    assert!(svc.update("tool-shout").await.is_err());
}

/// 开发态直载（不经 install → 无 lockfile 条目）的插件必须出现在
/// list_installed 投影里，来源标 Builtin（阶段八③用户反馈：工具可用
/// 但"插件"页看不见）。
#[tokio::test]
async fn registry_only_plugins_project_as_builtin() {
    let vers = staged_versions();
    if vers.is_empty() {
        eprintln!("skip: market/tool-shout 未暂存（先跑 scripts/build_market.sh）");
        return;
    }
    let data = temp_dir("builtin");
    let market_root = temp_dir("builtin-market");
    let registry = Arc::new(PluginRegistry::new());
    let ctx = Arc::new(Context::new());
    let svc = Arc::new(
        PluginService::open(
            data,
            Arc::clone(&registry),
            ctx,
            Box::new(LocalDirMarket::new(market_root)),
        )
        .unwrap(),
    );

    // 开发态装载路径：绕过 install，直接 load + activate（desktop 的
    // plugins/ 目录装载就是这个形态，§11.3 开发态不经 lockfile）
    let dll = repo_root()
        .join("market/tool-shout")
        .join(vers[0].to_string())
        .join(crate::artifact_file_name("tool-shout"));
    registry.load("tool-shout", &vers[0], &dll).unwrap();
    registry.activate("tool-shout", &vers[0]).unwrap();

    let listed = svc.list_installed();
    assert_eq!(listed.len(), 1, "registry-only 插件应出现在投影里");
    assert_eq!(listed[0].id, "tool-shout");
    assert_eq!(
        listed[0].source,
        crate::lockfile::InstallSource::Builtin,
        "无 lockfile 条目 = 内置来源"
    );
    assert_eq!(listed[0].version, vers[0].to_string());
    assert_eq!(listed[0].active_version, Some(vers[0].to_string()));
    assert!(listed[0].enabled, "active 即 enabled");
}
