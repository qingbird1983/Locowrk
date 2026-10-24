//! Tauri 命令薄适配层（DESIGN.md §10.3 / §12.1）：只做参数转换与状态
//! 存取，业务逻辑在 runner / approval / kernel，本文件不写一行。
//!
//! 台账第三节 · 第一批第一刀（519 → 本文件只留声明）：命令本体按**簇**沉进
//! `commands/` 同名目录，靠 `pub use X::*` 让公开面与此前逐字相同——`lib.rs` 的
//! `generate_handler!` 与所有消费点一个字都不用改。六个新片各管一件事：
//! `chat`（跑一轮 / 停一轮 / 回退压缩 / 会话列表与全量投影）、
//! `approvals`（审批决策 + 项目粘性授权规则）、
//! `settings`（读写 + 保存后的热更新广播）、
//! `probe`（就地探测与诊断读数，全部不落盘）、
//! `dialog`（OS 原生目录 / 存储 / 多选文件对话框）、
//! `notifications`（应用内通知读与置读）。
//! 原有各片（ask / assets / browser / llm / llm_import / plugins / schedule /
//! sessions）与 `#[cfg(test)]` 那条点名再导出不受本刀影响。S4：`organize` 片随
//! 宿主模块整面删除（八条树命令 + `/整理`，刀③b）；`memory` 片随记忆存储/注入
//! 同刀收口（十四条记忆命令，刀③c）。

mod approvals;
mod ask;
mod assets;
mod browser;
mod chat;
mod compact;
mod dialog;
mod files;
mod llm;
mod llm_import;
mod notifications;
mod plugins;
mod probe;
mod schedule;
mod sessions;
mod settings;

pub use approvals::*;
pub use ask::*;
pub use assets::*;
pub use browser::*;
pub use chat::*;
pub use compact::*;
pub use dialog::*;
pub use files::*;
pub use llm::*;
pub use llm_import::*;
pub use notifications::*;
pub use plugins::*;
pub use probe::*;
pub use schedule::*;
pub use sessions::*;
pub use settings::*;
// 台账 T6：导出标题的两块纯逻辑要被用例钉住——命令本体要 Tauri `State`，测试打不进去。
// 上面那个 `pub use sessions::*` glob **本来就会**把 `export_title` / `first_user_headline`
// 一起再导出：glob 按「再导出声明」与「项自身可见性」的较小者带项，`pub(crate)` 项照样
// 出来（收窄成 `pub(crate)`，crate 内的测试消费点照旧解析），且 glob 再导出不触发
// `unused_imports`（T27 两次注释实验实测：在 `sessions.rs` 与本文件各注释掉一次点名行，
// 都 rc=0）。曾在此放过的 `#[cfg(test)] pub(crate) use sessions::{…};` 点名行因此是
// 冗余而非必需，2026-10-24 随 T29 删除（判据 = `cargo clippy --all-targets` rc=0 且
// `tests_session_log/export_title.rs` 仍绿，specs/03 T29 条目）。
