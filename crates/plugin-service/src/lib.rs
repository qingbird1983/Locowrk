//! 插件安装/卸载/更新编排，CLI 与桌面端共用（DESIGN.md §11）。
//!
//! 职责划分：
//! - [`lockfile`]：已安装插件契约文件（安装态的磁盘真相）
//! - [`market`]：市场客户端抽象 + 本地目录市场（发现/下载）
//! - [`installer`]：包校验 + 解压落位（本地市场 = 目录复制）
//! - [`service`]：编排（install / set_enabled / uninstall / update / rollback
//!   / startup_reconcile），UI 状态广播经 `tokio::sync::broadcast`
//!
//! 纪律：UI 只通过本 crate 操作内核，不直接碰 `PluginRegistry`；
//! 安装经 `ctx.effect()` 登记 EffectId 写入 lockfile，卸载先逻辑卸载
//! （graveyard，永不 dlclose）再删文件，Windows 文件锁删不掉的走
//! tombstone，下次启动 `startup_reconcile` 清理。

pub mod builtin;
pub mod display;
pub mod installer;
pub mod lockfile;
pub mod market;
pub mod service;

pub use builtin::{dll_name, find_artifact, resolve_latest};
pub use display::{PluginDisplay, read_display};
pub use lockfile::{InstallSource, InstalledEntry, LockFile};
pub use market::{LocalDirMarket, Market, MarketplaceItem, artifact_file_name};
pub use service::{InstalledPlugin, PluginService, PluginStateChange};
