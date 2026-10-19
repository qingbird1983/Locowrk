//! LLM 主模型区（阶段八② / S2 窗口 / v2.58 档位制）。持久态 = `LlmConfig { active,
//! providers }`（DESIGN §13.1 模型档位制——多 provider 配置管理）；生效配置 = 活动档的
//! 平面投影 `LlmSettings`（字段语义与全部既有消费方不变）。旧平面字段由
//! `settings.rs::open` 迁移为单档 `default`（`migrate.rs`，一次幂等）。
//!
//! **审计 F5 拆分（v2.156）**：原 `settings/llm.rs` 682 行做了四件互不相干的事——
//! 落盘结构、校验规则、目录合成、IPC 形状（外加磁盘旧格式迁移）。三问自检里①②③
//! 全部命中（一个文件四件事 / 名字概括不了内容 / 改展示合成要滚过落盘结构），故按
//! 职责落成目录式子模块，**外部 API 一字未改**：`crate::settings::llm::X` 与
//! `settings.rs` 的 17 条 `pub use llm::{…}` 全部照常解析（那条 pub use 清单随批实测
//! 为 17，此前文档与提交说明里的「14 条」是失实自报，见《AUDIT-FIX-PLAN-v1》§5 批 F-6）。
//!
//! 搬运是机械的：每个顶层项（含其紧邻文档注释与属性行）整块搬进对应簇文件，块内字节
//! 原样、簇内保持原相对次序；判据脚本 `tmp/v156-*` 留在台账里可复跑。
//!
//! **一处如实的路径下沉**：原文件里 `RESERVED_HEADERS` / `MAX_CUSTOM_HEADERS` /
//! `PROXY_MODES` / `PricingView` / `SourcesView` 五项是 `pub`，但 `settings.rs` 的
//! `pub use llm::{…}` 从没带过它们，而 `mod llm;` 本身是 `settings` 的私有子模块 ⇒
//! 全仓零引用（脚本对账）。拆簇后它们保持 `pub` 留在各自簇内（`PricingView`/`SourcesView`
//! 是 `pub` 结构体的字段类型，必须够宽），**不再在 mod.rs 里空转 re-export**——那样
//! rustc 会报 `unused_imports`（私模块里的 pub use 无人可达），为了维持一条没人走的路径
//! 去挂 `#[allow]` 正是批 A 在清的形态。真实可达路径由 `llm::X` 变为 `llm::policy::X`
//! 一级，实测引用点为 0，所以外部 API 的**使用面**一字未改。

mod ipc;
mod migrate;
mod persist;
mod policy;
mod synthesize;

pub use ipc::{LlmSettingsView, LlmUpdate, ProviderInput, ProviderView};
pub(super) use migrate::migrate_flat_llm;
pub use persist::{
    new_profile_id, session_capacity, session_window, LlmConfig, LlmSettings, ProviderModel,
    ProviderProfile, SubagentPin,
};
pub use policy::{validate_headers, NetworkProxy, PROVIDERS};
pub use synthesize::{synthesize_view, synthesize_view_raw, ProviderModelView};
