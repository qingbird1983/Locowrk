//! 插件宿主：dlopen Loader / 多版本 Registry / FFI→Tool 适配（DESIGN.md §5）。
//!
//! 计划模块：`loader`（DylibLoader，WasmLoader 预留）、`registry`（多版本并存 +
//! graveyard）、`adapter`（SabiToolAdapter）。
//!
//! 纪律：
//! - 永不 dlclose：逻辑卸载 + 物理保留，进程重启才真正释放
//! - FFI 调用放 `spawn_blocking`，插件与宿主两侧都要 `catch_unwind`
//! - 在途调用靠 `Arc` 引用计数，drain 时轮询 `Arc::strong_count`
