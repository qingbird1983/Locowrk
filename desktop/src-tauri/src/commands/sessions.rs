//! 会话运营命令（v2.49，UI shell 文档 §3.6 行尾 `…` 菜单的后端）：重命名
//! / 删除 / 搜索 / 导出。§3.16 子转录读取（v2.52）。薄适配层——文件面在
//! session_store::persist（经 session_log 薄封装），投影与渲染在 runner /
//! export，这里只接参数。
//!
//! 台账第三节 · 第一批第二刀（474 → 本文件只留声明）：命令本体按簇沉进
//! `sessions/` 目录，靠 `pub use X::*` 保持公开面与此前逐字相同——`lib.rs` 的
//! `generate_handler!` 与所有消费点一个字不改。六片各管一件事：
//! `meta`（元数据写侧：改名 / 归属 / 权限档 / 模型与推理档覆盖）、
//! `lifecycle`（删除与正文搜索）、
//! `export`（导出 Markdown 与标题三层规则）、
//! `heal`（截断回退与崩溃后悬空调用裁决）、
//! `transcript`（子 Agent 转录读取）、
//! `shell`（打开目录 / 打开链接的 OS 外壳）。

mod export;
mod heal;
mod lifecycle;
mod meta;
mod shell;
mod transcript;

pub use export::*;
pub use heal::*;
pub use lifecycle::*;
pub use meta::*;
pub use shell::*;
pub use transcript::*;
// 本片三个 `pub(crate)` 项（`export_title` / `first_user_headline` / `ensure_associatable`）
// 由上面的 glob 按**收窄后的可见性**带出，不再点名再导出（点名 = 同一事实第二处出处）。
// 机制是本刀实测的：把点名行注释掉后 `cargo clippy --all-targets -- -D warnings` 仍 rc=0，
// 消费点 `crate::commands::sessions::ensure_associatable` 照旧解析。读数和坑记在
// `specs/03-structural-debt/spec.md` 的 T27 条目（同批登记的 T28 = `commands.rs` 里那条
// 旧注释把机制说反了，待单独一刀更正）。
