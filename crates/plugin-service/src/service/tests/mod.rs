//! 编排层测试。需要真实 DLL 的用例依赖 `scripts/build_market.sh` 暂存的
//! `market/tool-shout/{ver}/`（工作区根），缺失时打印 skip 并通过——
//! 保证"裸 cargo test"与"完整验收"两种环境都可运行。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use cordis_core::Context;
use plugin_host::PluginRegistry;
use semver::Version;

use super::{PluginService, PluginStateChange};
use crate::lockfile::LockFile;
use crate::market::LocalDirMarket;

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
    // packaging.md v2 全量 manifest（installer/registry 都按它解析）
    let manifest = format!(
        r#"{{"id":"{id}","version":"{v}","abi_version":1,"kind":"tool",
            "display_name":{{"en-US":"Shout"}},
            "description":{{"en-US":"{description}"}},
            "origin":"official",
            "tools":[{{"name":"shout","description":"d","schema":{{}},"side_effect":"Pure"}}]}}"#
    );
    std::fs::write(dir.join("manifest.json"), manifest).unwrap();
}

mod fallback;
mod lifecycle;
