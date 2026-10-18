//! LLM 档位与模型清单命令（v2.58，DESIGN §13.1 档位制）：探测（选择器
//! 数据源）+ 档位增删改 + 模型能力目录三件（批 A：状态/刷新/单模型查
//! 询）。命令薄适配——校验在 settings/llm.rs，HTTP 在 llm/models.rs，
//! 目录数据在 llm/catalog.rs，合成在 settings/llm.rs `synthesize_view`。
//!
//! 台账第三节 · 第一批第三刀（523 → 本文件只留声明）：命令本体按簇沉进
//! `llm/` 目录，靠 `pub use X::*` 保持公开面与此前逐字相同——`lib.rs` 的
//! `generate_handler!` 与 `commands/settings.rs` 的 `use super::llm::enrich_with_catalog`
//! 一个字都不用改。六片 = `views`（IPC 视图与目录合成的共读段）· `models`（选择器清单）·
//! `probe`（表单探测与推荐三选）· `test`（连通测试两路）· `providers`（代理与档位 CRUD）·
//! `catalog`（能力目录三件与缓存诊断）。
//! ⚠ 判定表那行的四簇切法（清单+推荐 / 连通测试 / 代理与档位 CRUD / 目录诊断）在本刀拆成六片：
//! `清单` 是全档并行、`推荐` 是单档扫描加预选，合成一片违三问 ①；两处共读的
//! `enrich_with_catalog` 与 `apply_dialect_ladder`（原 44/298/478 三个引用点）单立 `views` 片，
//! 不塞进任何一个命令片当"顺手的公共文件"。

mod catalog;
mod models;
mod probe;
mod providers;
mod test;
mod views;

pub use catalog::*;
pub use models::*;
pub use probe::*;
pub use providers::*;
pub use test::*;
pub use views::*;
