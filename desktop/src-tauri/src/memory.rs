//! 记忆库宿主侧访问（⑪）：真源 = `<数据根>/memory/` 下的一事实一文件
//! （《Memory File-ization Design v1.1》，S1 刀② 扳的正是这一句）；本模块做两类宿主
//! 自有访问——
//! ① `MemorySource` 实现：Memory-Inject（kernel-agent）每 step 的检索源。
//!    不走工具管道（否则每 step 一次 memory_list 会污染使用统计与审计），
//!    直读真源（S3 刀① 之后注入面只有**两层索引**：`global_index` 与 `project_index`，
//!    都是文件面——正文一个字都不进 system）。
//! ② 记忆管理页的存储支撑（Context and Memory.md §7.2）：列举 / 编辑 /
//!    删除 / 清空——Tauri 命令薄适配到这里，命令层不碰落盘细节。
//!
//! ⚠ **S3 刀③b 把上面那句"尚未收口的例外"收掉了**：学习档 `recall_relevant` 与它的
//! `memory_learn/` 整族（四信号加权排序 + 交付即写回）按裁定退役（specs/10 开工三问②）
//! ⇒ **本模块现在一条 SELECT 都不发**：`recall`、两层索引、管理页列举全部读 md 真源。
//! `memory.sqlite3` 剩下的只有对账的写侧与两张遗列（`hit_count` / `last_hit_at`：
//! 在场、无人读写，去处随 S4 一起裁——specs/05 刀②c-2 那一格至此**有了答案**，
//! 挂账在那份 spec 里订正为"已退役，列待 S4"）。
//!
//! source 列（explicit | extracted，§4.5 管理页要显示来源）归宿主侧
//! 演进；workspace 列（㉘，项目隔离键位）建表即带，旧库由
//! `ensure_workspace_schema` 整表重建进未归属键位——**这份迁移现在只有宿主一侧**
//! （插件侧那两份随刀③b-1 的 `store/schema.rs` 一起退役，两侧对账闸同批改成单侧
//! H1–H7，见 `check-schema-parity.mjs` 头注）。
//!
//! **注入面（S3 刀① 换过形状）**：进 system 的是**两层索引的指针行**——`global_index`
//! 与 `project_index` 合成一个 `<memory_index>` 块，正文一个字都不进。所以
//! v2.136 那句「Memory-Inject 只取全局层」在这里**已过期**（别照它推"项目层不进上下文"），
//! 而它当初禁的那件事（把事实全文灌进 system）仍然成立：索引给的是"记了哪些条、各在
//! 哪一篇"，要正文就按指针读那篇文件。项目交接层另走 `<repo_context>`（S3 刀② 起真读
//! `<repo>/.Locowrk/MEMORY.md` 的现状正文，不再是静态提醒）。管理页是人的
//! 管理面，看全部并透出归属。降级纪律：真源读不动 = 检索降级空表（仅告警
//! 一次，注入不拦聊天）+ 写路径报错；库里那份是影子，它坏了不影响读面也不让写失败——
//! 刀③b-2 之后写路径**根本不开这张表**（四条 `shadow_*` 退役，影子的内容列只由
//! `memory_reconcile` 从真源重建）。不在启动路径上 panic。
//!
//! 台账第三节 · 第二批第一刀（916 → 本文件只留声明）：宿主侧访问按**面**沉进 `memory/`，
//! 靠 `pub use X::*` 让 `crate::memory::*` 的公开面与此前逐字相同——`pub(crate)` 项
//! （`KEY_MAX` / `VALUE_MAX`）由 glob 按**收窄后的可见性**带出，不点名再导出。
//! 八片 = `shapes`（上限常量与行/表形状）· `store`（结构与装配 + 开库、行装配）·
//! `crud`（管理页五条读写）· `reconcile`（首启导出与对账直写）· `recall`（全局层召回与
//! 埋点）· `schema`（旧库迁移两条）· `source`（`MemorySource` 实现与组装点便利函数）·
//! `layers`（S1 刀④c：每层一行与回收区的读数装配，一片自由函数）。
//! `impl MemoryStore` 分进四个文件 ⇒ 原文那对花括号只归两处（开在 `store`、合在 `recall`），
//! 于是 `store` 补一条合、`crud` 与 `reconcile` 各补一对、`recall` 补一条开 = 共 6 行外壳，
//! 逐条声明在 spec 的 `wrap` 里，对账先剥掉它再比连续与守恒。
//! 片与片之间只靠"抬到 `pub(super)`"这一种通道相连，14 处逐条枚举在本刀的 spec 的
//! `edits` 里，对账判据吃"把这些改动施加之后的基线"——多一处、少一处、改一处都会红。
//! `mod tests;` 留在本文件：用例是 `memory` 的子模块，它吃的正是上面三个 glob 带出来的名字。

mod crud;
mod layers;
mod recall;
mod reconcile;
mod schema;
mod shapes;
mod source;
mod store;

pub use layers::*;
pub use shapes::*;
pub use source::*;
pub use store::*;

#[cfg(test)]
mod tests;
