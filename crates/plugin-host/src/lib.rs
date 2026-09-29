//! 插件宿主：dlopen Loader / 多版本 Registry / FFI→Tool 适配（DESIGN.md §5）。
//!
//! 模块：
//! - `loader`：DylibLoader（dlopen + describe + ABI 校验 + init；WasmLoader 预留）
//! - `instance`：PluginInstance（FFI 调用 + Drop = shutdown → dlclose）
//! - `host_vtable`：宿主能力表（'static，init 交付给插件）
//! - `registry`：多版本并存 + active 指针 + graveyard（§5.7）
//! - `adapter`：SabiToolAdapter（FFI → Tool）+ RegistryToolSource（工具目录）
//! - `error`：LoadError
//!
//! 纪律：
//! - 永不 dlclose：逻辑卸载 + 物理保留，进程重启才真正释放（§5.6 纪律一）。
//!   全进程唯一的 dlclose 路径是 PluginInstance::drop，正常运转时不可达
//!   （registry 只把 Arc 挪 graveyard）；可达的只有"加载即失败"豁免（附录 C-3）
//! - FFI 调用放 `spawn_blocking`，插件与宿主两侧都 `catch_unwind`（纪律 3）
//! - 在途调用靠 `Arc` 引用计数，drain 时轮询 `Arc::strong_count`（§9.6）
//!
//! UI 状态变化广播落在 plugin-service（编排层，单一广播源）：注册表变更
//! 在本架构下都经编排层发生，启动期开发态装载发生在 UI 订阅之前，
//! 由 list_installed 拉取兜底。

mod adapter;
mod error;
mod host_vtable;
mod instance;
mod loader;
mod registry;

pub use adapter::{RegistryToolSource, SabiToolAdapter};
pub use error::LoadError;
pub use instance::PluginInstance;
pub use loader::DylibLoader;
pub use registry::{PluginRegistry, PluginStateSummary};
