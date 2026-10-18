//! 记忆库宿主侧访问（⑪）：`<数据根>/memory.sqlite3` 归 memory-sqlite
//! 插件所有（schema 权威在两侧同源迁移）；本模块做两类宿主自有访问——
//! ① `MemorySource` 实现：Memory-Inject（kernel-agent）每 step 的检索源。
//!    不走工具管道（否则每 step 一次 memory_list 会污染使用统计与审计），
//!    直读同一 sqlite 文件（两侧都是短连接，busy_timeout 兜底并发）。
//! ② 记忆管理页的存储支撑（Context and Memory.md §7.2）：列举 / 编辑 /
//!    删除 / 清空——Tauri 命令薄适配到这里，命令层不写 SQL。
//!
//! source 列（explicit | extracted，§4.5 管理页要显示来源）归宿主侧
//! 演进；workspace 列（㉘，项目隔离键位）两侧同形状——建表即带，旧库
//! 由 `ensure_workspace_schema` 整表重建进未归属键位（与插件侧迁移
//! 等价幂等，先跑谁都一样）。
//!
//! **注入面（v2.136 裁定「项目侧全提醒制」）**：Memory-Inject 只取
//! **全局层**（`''` = 用户层面的喜好/全局要求，对一切会话可见；未归属
//! 会话同样只落全局层）——项目层事实不再自动注入，由模型 memory_read /
//! memory_search 按需查；项目记忆文件（`<repo>/.Locowrk/`）以静态提醒段
//! 指路（repo_context），由当前工作的主模型自己读/写/改。管理页是人的
//! 管理面，看全部并透出归属。降级纪律：打不开 = 检索降级空表（仅告警
//! 一次，注入不拦聊天）+ 管理命令报错给 UI；不在启动路径上 panic。
//!
//! 台账第三节 · 第二批第一刀（916 → 本文件只留声明）：宿主侧访问按**面**沉进 `memory/`，
//! 靠 `pub use X::*` 让 `crate::memory::*` 的公开面与此前逐字相同——`pub(crate)` 项
//! （`KEY_MAX` / `VALUE_MAX` / `FactLine`）由 glob 按**收窄后的可见性**带出，不点名再导出。
//! 七片 = `shapes`（上限常量与行/表形状）· `store`（结构与装配 + 开库、行装配）·
//! `crud`（管理页五条读写）· `reconcile`（首启导出与对账直写）· `recall`（全局层召回与
//! 埋点）· `schema`（旧库迁移两条）· `source`（`MemorySource` 实现与组装点便利函数）。
//! `impl MemoryStore` 分进四个文件 ⇒ 原文那对花括号只归两处（开在 `store`、合在 `recall`），
//! 于是 `store` 补一条合、`crud` 与 `reconcile` 各补一对、`recall` 补一条开 = 共 6 行外壳，
//! 逐条声明在 spec 的 `wrap` 里，对账先剥掉它再比连续与守恒。
//! 片与片之间只靠"抬到 `pub(super)`"这一种通道相连，14 处逐条枚举在本刀的 spec 的
//! `edits` 里，对账判据吃"把这些改动施加之后的基线"——多一处、少一处、改一处都会红。
//! `mod tests;` 留在本文件：用例是 `memory` 的子模块，它吃的正是上面三个 glob 带出来的名字。

mod crud;
mod recall;
mod reconcile;
mod schema;
mod shapes;
mod source;
mod store;

pub use shapes::*;
pub use source::*;
pub use store::*;

#[cfg(test)]
mod tests;
