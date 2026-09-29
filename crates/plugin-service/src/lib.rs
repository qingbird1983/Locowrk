//! 插件安装/卸载/更新编排，CLI 与桌面端共用（DESIGN.md §11）。
//!
//! 计划模块：`service`（install/enable/uninstall/update/rollback）、
//! `lockfile`（已安装插件契约文件）、`market`（市场客户端）、
//! `installer`（下载 + 校验 + 解压）。
//!
//! 纪律：UI 只通过本 crate 操作内核，不直接碰 `PluginRegistry`；
//! 安装经 `ctx.effect()` 注册 EffectId 写入 lockfile，卸载先撤销 Effect 再删文件。
