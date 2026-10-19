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
//! 原有十片（ask / assets / browser / llm / llm_import / memory / organize /
//! plugins / schedule / sessions）与 `#[cfg(test)]` 那条点名再导出不受本刀影响。

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
mod memory;
mod notifications;
mod organize;
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
pub use memory::*;
pub use notifications::*;
pub use organize::*;
pub use plugins::*;
pub use probe::*;
pub use schedule::*;
pub use sessions::*;
pub use settings::*;
// 台账 T6：导出标题的两块纯逻辑要被用例钉住——命令本体要 Tauri `State`，测试打不进去。
// `pub use sessions::*` 只带得出 `pub` 项，这两个是 `pub(crate)`，所以在这里点名再导出，
// 不把命令层内部件顺手抬成公开 API。⚠ 必须收在 `#[cfg(test)]` 里：生产面没有人用它们，
// 不带 cfg 的话非测试编译报 `unused_imports`，`-D warnings` 当场红（`fence` 那两个再导出
// 分档是同一课，见 `approval/mod.rs` 的注释）。
#[cfg(test)]
pub(crate) use sessions::{export_title, first_user_headline};
