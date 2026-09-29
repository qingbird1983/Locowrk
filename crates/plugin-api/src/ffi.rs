//! FFI 边界（DESIGN.md §5.1）。冻结契约：这里的一切一经发布不可再改签名，
//! 加字段/加导出只能追加。边界上只传 C 类型 + 字节串，Rust 类型（如
//! `PluginManifest`）仅在两侧各自序列化/反序列化，绝不跨边界传递。

use std::ffi::c_void;

/// FFI 返回码（跨边界只传 `i32`；非零即失败）。
///
/// 约定：call 失败时 `out` 缓冲尽力携带 `{"error": "<message>"}` JSON，
/// 宿主优先从缓冲取错误信息，取不到再退回按码映射。
pub mod code {
    pub const OK: i32 = 0;
    /// 插件内部 panic（已被插件侧 `catch_unwind` 拦下，纪律 3 / §9.4）。
    pub const ERR_PANIC: i32 = -1;
    /// 请求的工具名在本插件中不存在。
    pub const ERR_UNKNOWN_TOOL: i32 = -2;
    /// 工具执行失败（`out` 携带错误信封）。
    pub const ERR_CALL_FAILED: i32 = -3;
    /// 参数字节串不可解析（UTF-8 / JSON 层面）。
    pub const ERR_BAD_ARGS: i32 = -4;
    /// init 传回的句柄为空或非法。
    pub const ERR_BAD_HANDLE: i32 = -5;
    /// 宿主/插件能力未实现（如阶段三的 `kv_get`，见 host_vtable 注释）。
    pub const ERR_UNSUPPORTED: i32 = -6;
}

/// `HostVtable::log` 的日志级别（语义对齐 tracing）。
pub mod log_level {
    pub const ERROR: u32 = 1;
    pub const WARN: u32 = 2;
    pub const INFO: u32 = 3;
    pub const DEBUG: u32 = 4;
}

/// 插件必须导出的五个符号（NUL 结尾字节串，直接喂 `Library::get`，§5.1）。
pub const SYM_DESCRIBE: &[u8] = b"agent_plugin_describe\0";
pub const SYM_INIT: &[u8] = b"agent_plugin_init\0";
pub const SYM_CALL: &[u8] = b"agent_plugin_call\0";
pub const SYM_SHUTDOWN: &[u8] = b"agent_plugin_shutdown\0";
pub const SYM_FREE: &[u8] = b"agent_plugin_free_buffer\0";

/// describe → NUL 结尾 UTF-8 JSON（`PluginManifest`）。
///
/// 设计补充（§5.1 未指明长度协议）：JSON 文本本身不含裸 NUL（serde 会转义），
/// 故以 NUL 结尾传递长度自描述，宿主按 C 字符串读取。Rust 插件用
/// `PluginManifest::to_describe_json` 生成，宿主用 `PluginManifest::from_json` 解析。
pub type DescribeFn = extern "C" fn() -> *const u8;

/// init → 插件自管句柄（内容对宿主不透明；插件侧约定见 `declare_plugin!`）。
/// 返回空指针 = init 失败，宿主拒绝加载（"加载即失败"豁免，§5.7）。
pub type InitFn = extern "C" fn(host: *const HostVtable) -> *mut c_void;

/// call：入参/出参都是 UTF-8 JSON 字节串。
///
/// 缓冲区所有权：`out` 由**插件**分配（`std::alloc`，对齐 1、长度即 out_len），
/// 宿主用完后**必须**经插件的 `agent_plugin_free_buffer` 释放——两侧可能是
/// 不同的分配器，禁止跨边界 free。
pub type CallFn = extern "C" fn(
    handle: *mut c_void,
    tool: *const u8,
    tool_len: usize,
    args: *const u8,
    args_len: usize,
    out: *mut *mut u8,
    out_len: *mut usize,
) -> i32;

/// shutdown：释放 init 返回的句柄，此后句柄不可再用于 call。
pub type ShutdownFn = extern "C" fn(handle: *mut c_void);

/// free_buffer：释放本插件经 `out`/`kv_get` 交出的缓冲（布局：对齐 1、长度即 len）。
pub type FreeBufFn = extern "C" fn(ptr: *mut u8, len: usize);

/// 宿主能力表（§5.1）。init 时交给插件；指针必须 'static
/// （宿主侧提供静态实例，插件只持有借用）。
///
/// 追加规则：新增能力只能加在结构体**末尾**（§15.1），插件应容忍
/// "只拿到前缀字段"的旧宿主——阶段三的全部字段为必填。
#[repr(C)]
pub struct HostVtable {
    pub log: extern "C" fn(level: u32, ptr: *const u8, len: usize),
    pub kv_get:
        extern "C" fn(k: *const u8, klen: usize, out: *mut *mut u8, olen: *mut usize) -> i32,
    pub kv_put: extern "C" fn(k: *const u8, klen: usize, v: *const u8, vlen: usize) -> i32,
}

impl HostVtable {
    /// 插件侧便捷封装：把 UTF-8 字符串经 vtable 打日志。
    ///
    /// # Safety
    /// `self` 必须是 init 时宿主交付的合法指针。
    pub unsafe fn log_str(&self, level: u32, msg: &str) {
        (self.log)(level, msg.as_ptr(), msg.len());
    }
}

#[cfg(test)]
mod tests;
