//! 组装点·插件编排簇（DESIGN §11.2 + §12.3）：服务开启（含数据根降级）→ 启动
//! 对账 → 开发态装载 → 状态广播 → 自动停用巡检 → 返回 `Arc<PluginService>`。
//!
//! 本簇语句自 `state/mod.rs` 的 `init_state` **原样搬出**（批 F-3a·F5 结构性
//! 拆分），**内部顺序就是启动顺序，一条未重排**——这条纪律的意义：
//! 1. `startup_reconcile` 必须先于任何插件加载（清 tombstone / lockfile 与磁盘
//!    对账时文件锁还不成立，§11.2），顺序错 = 恢复逻辑对不上账；
//! 2. `builtin_root` 一次解析，服务（启停时解析工件路径）与开发态扫描共用（S2），
//!    两处各解析一次会在环境覆盖下取到不同根；
//! 3. 自动停用巡检放最后并 `tauri::async_runtime::spawn` 后台化——setup 主线程
//!    没有 tokio 上下文，`tokio::spawn` 直接 panic（v2.130 走查实抓）。
//!
//! UI 永远不直接碰 `PluginRegistry`（AGENTS 架构边界）：注册表在这里只作为
//! 服务与开发态装载的入参穿过，装载结果经 `plugins.register_builtin` 落编排层。

use std::path::PathBuf;
use std::sync::Arc;

use cordis_core::Context;
use plugin_host::PluginRegistry;
use plugin_service::{LocalDirMarket, PluginService};
use tauri::AppHandle;

use crate::data_root;
use crate::events::forward_plugin_states;
use crate::plugins_root;
use crate::settings::SettingsStore;

/// `open_and_load` 的入参：跨簇共享的启动期句柄（逐字段显名传，位置错不了）。
pub(crate) struct Init {
    pub app: AppHandle,
    pub ctx: Arc<Context>,
    pub registry: Arc<PluginRegistry>,
    pub settings: Arc<SettingsStore>,
    pub usage: Arc<crate::usage::UsageRecorder>,
    pub root: PathBuf,
}

/// 打开插件服务并完成装载三段（对账 / 开发态 / 广播），返回服务句柄。
pub(crate) fn open_and_load(init: Init) -> Arc<PluginService> {
    let Init {
        app,
        ctx,
        registry,
        settings,
        usage,
        root,
    } = init;

    // 插件编排（§11.2）：数据根缺省 ~/.Locowrk（LOCOWRK_DATA_DIR可隔离）；打不开时退化到临时目录并明确告警——聊天不受影响。
    // S2：内置插件根一次解析，服务（启停解析工件）与开发态扫描共用。
    let builtin_root = plugins_root::resolve();
    let plugins =
        match open_plugin_service(&root, Arc::clone(&registry), &ctx, builtin_root.clone()) {
            Ok(svc) => Arc::new(svc),
            Err(primary_err) => {
                tracing::error!("plugin service init failed: {primary_err}；降级到临时目录");
                let fallback = std::env::temp_dir().join("locowrk-degraded-data");
                match open_plugin_service(
                    &fallback,
                    Arc::clone(&registry),
                    &ctx,
                    builtin_root.clone(),
                ) {
                    Ok(svc) => Arc::new(svc),
                    Err(e) => panic!("plugin service unavailable even in fallback dir: {e}"),
                }
            }
        };

    // 1. 启动对账（任何插件加载之前）
    if let Err(e) = plugins.startup_reconcile() {
        tracing::warn!("startup reconcile failed: {e}");
    }
    // 2. 开发态装载 plugins/（找不到不致命）。S2：跳过 lockfile 已接管/已停用 id；装载即登记 Builtin 条目（停用落 lockfile，重启不复活）。
    match builtin_root {
        Some(proot) => {
            let skip: std::collections::HashSet<String> =
                plugins.dev_scan_skip_ids().into_iter().collect();
            match plugins_root::load_all(&proot, &registry, &skip) {
                Ok(loaded) if loaded.is_empty() => {
                    tracing::warn!(dir = %proot.display(), "plugins/ 存在但没有加载任何插件")
                }
                Ok(loaded) => {
                    for (id, ver) in &loaded {
                        plugins.register_builtin(id, ver);
                    }
                    tracing::info!(dir = %proot.display(), ?loaded, "plugins loaded")
                }
                Err(e) => tracing::warn!("plugin scan failed: {e}"),
            }
        }
        None => tracing::warn!("plugins/ 目录未找到（可设 LOCOWRK_PLUGINS_DIR）；工具调用将不可用"),
    }
    // 3. 安装态已在对账中恢复；4. 状态广播转发给 UI（§13.4）
    forward_plugin_states(app.clone(), plugins.subscribe());

    // 5. ⑬ 自动停用巡检（exposure 文档 §8.8 例外条款，缺省关；spawn 化不阻塞窗口出现）
    let days = settings.plugins().auto_disable_idle_days;
    if days > 0 {
        let plugins2 = Arc::clone(&plugins);
        let usage2 = Arc::clone(&usage);
        // setup 主线程没有 tokio 上下文，后台化走 tauri 全局运行时（同 events / usage / scheduler-engine 先例）。
        tauri::async_runtime::spawn(async move {
            crate::autodisable::auto_disable_idle(&plugins2, &usage2, days).await;
        });
    }

    plugins
}

fn open_plugin_service(
    data_dir: &std::path::Path,
    registry: Arc<PluginRegistry>,
    ctx: &Arc<Context>,
    builtin_root: Option<std::path::PathBuf>,
) -> Result<PluginService, String> {
    let market_root =
        data_root::resolve_market_dir().unwrap_or_else(|| data_dir.join("market-absent"));
    tracing::info!(
        data = %data_dir.display(),
        market = %market_root.display(),
        "plugin service opening"
    );
    PluginService::open(
        data_dir.to_path_buf(),
        registry,
        Arc::clone(ctx),
        Box::new(LocalDirMarket::new(market_root)),
        builtin_root,
    )
}
