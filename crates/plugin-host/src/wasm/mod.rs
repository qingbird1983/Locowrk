//! WASM 插件支持（DESIGN.md §5.8 第二实现）：wasmtime 沙箱加载器。
//!
//! - `abi`：客户机导出 / 宿主导入约定与沙箱预算常量
//! - `loader`：WasmLoader（编译 + 受限实例化 + describe/init 握手）
//! - `backend`：PluginBackend 实现（同实例 Mutex 串行 + epoch 时间预算）
//!
//! 沙箱面：无 WASI——客户机拿不到文件/网络/env/时钟；内存与表经
//! Store limiter 限额；每次 call 有 epoch 时间预算。超限一律 trap，
//! 宿主映射为 LoadError / ToolError，绝不 panic。

mod abi;
mod backend;
#[cfg(test)]
mod guests;
mod loader;
#[cfg(test)]
mod tests;

pub use loader::WasmLoader;
