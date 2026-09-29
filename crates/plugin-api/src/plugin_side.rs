//! 插件侧 Rust 便利层（§5.3）：`PluginState` trait + `declare_plugin!` 宏 +
//! 缓冲区辅助。这一层**不进入 C ABI**（同 `tool` 模块的定位）——它只是把
//! §5.4 的手写五符号模板代码生成出来，编译产物与手写完全一致；
//! C/Zig 等其他语言插件直接手写五个符号即可，不经过本模块。

use std::alloc::{Layout, alloc, dealloc};
use std::ptr;

use serde_json::Value;

use crate::ffi::code;

/// 插件状态：Rust 插件实现它，`declare_plugin!` 生成把
/// `agent_plugin_call` 分派到 `PluginState::call` 的符号。
/// 错误以字符串信封回传（宿主侧映射为 `ToolError::Failed`）。
pub trait PluginState: Send + Sync + 'static {
    fn call(&self, tool: &str, args: Value) -> Result<Value, String>;
}

/// 按 FFI 约定分配缓冲并写入 `bytes`（§5.1 CallFn 契约：对齐 1、
/// 长度即 `*out_len`，宿主经插件的 `agent_plugin_free_buffer` 释放）。
/// 分配失败时返回 [`code::ERR_CALL_FAILED`] 且不动 out 参数。
///
/// # Safety
/// `out` / `out_len` 必须指向宿主栈上合法的可写槽位（CallFn 契约）。
pub unsafe fn write_out(bytes: &[u8], out: *mut *mut u8, out_len: *mut usize) -> i32 {
    if bytes.is_empty() {
        // 空缓冲：约定以空指针 + 长度 0 表达，宿主按"无输出"处理
        // SAFETY: 调用方契约保证两个槽位可写
        unsafe {
            *out = ptr::null_mut();
            *out_len = 0;
        }
        return code::OK;
    }
    let layout = Layout::from_size_align(bytes.len(), 1).expect("non-zero byte buffer");
    // SAFETY: layout 大小非零且对齐为 1
    let ptr = unsafe { alloc(layout) };
    if ptr.is_null() {
        return code::ERR_CALL_FAILED;
    }
    // SAFETY: ptr 刚按 layout 分配，容量与 bytes.len() 一致；槽位可写
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
        *out = ptr;
        *out_len = bytes.len();
    }
    code::OK
}

/// 释放 [`write_out`] 分配的缓冲；插件侧 `agent_plugin_free_buffer` 的实现体。
///
/// # Safety
/// `(ptr, len)` 必须来自本插件 `write_out` 的输出（或为空指针，此时无操作）。
pub unsafe fn free_allocated(ptr: *mut u8, len: usize) {
    if ptr.is_null() || len == 0 {
        return;
    }
    // SAFETY: 调用方契约保证 (ptr, len) 与 write_out 的分配一一对应
    unsafe { dealloc(ptr, Layout::from_size_align_unchecked(len, 1)) };
}

/// 生成插件的五个导出符号（§5.4）。
///
/// 用法：`declare_plugin!(describe_fn, make_state_fn);`
/// - `describe_fn: fn() -> std::ffi::CString`——返回 `PluginManifest::to_describe_json()`
/// - `make_state_fn: fn(*const HostVtable) -> Box<dyn PluginState>`
///
/// 约定：句柄 = `Box<Box<dyn PluginState>>` 的裸指针（外层 Box 抹平
/// trait 对象的胖指针，才能安全地经 `*mut c_void` 往返）。每个 crate
/// 只能调用一次本宏（符号名固定）。
#[macro_export]
macro_rules! declare_plugin {
    ($describe:expr, $make:expr) => {
        static __LOCOWRK_MANIFEST: std::sync::LazyLock<std::ffi::CString> =
            std::sync::LazyLock::new($describe);

        #[unsafe(no_mangle)]
        pub extern "C" fn agent_plugin_describe() -> *const u8 {
            __LOCOWRK_MANIFEST.as_ptr() as *const u8
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn agent_plugin_init(
            host: *const $crate::ffi::HostVtable,
        ) -> *mut std::ffi::c_void {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $make(host))) {
                Ok(state) => {
                    let boxed: Box<dyn $crate::plugin_side::PluginState> = state;
                    Box::into_raw(Box::new(boxed)) as *mut std::ffi::c_void
                }
                Err(_) => std::ptr::null_mut(),
            }
        }

        // 生成符号按 C 惯例保留 `extern "C"`（C 侧看不见 Rust 的 unsafe 标记）；
        // 裸指针解引用的契约已在各符号文档与 §5.1 写明，此处豁免该 lint。
        #[unsafe(no_mangle)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn agent_plugin_call(
            handle: *mut std::ffi::c_void,
            tool: *const u8,
            tool_len: usize,
            args: *const u8,
            args_len: usize,
            out: *mut *mut u8,
            out_len: *mut usize,
        ) -> i32 {
            let run = std::panic::AssertUnwindSafe(|| {
                if handle.is_null() {
                    return $crate::ffi::code::ERR_BAD_HANDLE;
                }
                let tool_name = (|| {
                    // SAFETY: 宿主契约保证 (tool, tool_len) 指向可读内存
                    let bytes =
                        unsafe { std::slice::from_raw_parts(tool, tool_len) };
                    std::str::from_utf8(bytes).ok()
                })();
                let tool_name = match tool_name {
                    Some(t) => t,
                    None => return $crate::ffi::code::ERR_BAD_ARGS,
                };
                let args_value = (|| {
                    // SAFETY: 宿主契约保证 (args, args_len) 指向可读内存
                    let bytes =
                        unsafe { std::slice::from_raw_parts(args, args_len) };
                    $crate::serde_json::from_slice::<$crate::serde_json::Value>(bytes).ok()
                })();
                let args_value = match args_value {
                    Some(v) => v,
                    None => return $crate::ffi::code::ERR_BAD_ARGS,
                };
                // SAFETY: handle 来自 init，且 shutdown 前宿主不会再次交还
                let state: &dyn $crate::plugin_side::PluginState = unsafe {
                    &**(handle as *const Box<dyn $crate::plugin_side::PluginState>)
                };
                match state.call(tool_name, args_value) {
                    Ok(value) => {
                        let bytes = $crate::serde_json::to_vec(&value)
                            .unwrap_or_default();
                        // SAFETY: out/out_len 是宿主按 CallFn 契约提供的槽位
                        unsafe { $crate::plugin_side::write_out(&bytes, out, out_len) }
                    }
                    Err(message) => {
                        let bytes = $crate::serde_json::json!({ "error": message })
                            .to_string()
                            .into_bytes();
                        // SAFETY: 同上
                        let code = unsafe { $crate::plugin_side::write_out(&bytes, out, out_len) };
                        if code == $crate::ffi::code::OK {
                            $crate::ffi::code::ERR_CALL_FAILED
                        } else {
                            code
                        }
                    }
                }
            });
            match std::panic::catch_unwind(run) {
                Ok(code) => code,
                Err(_) => $crate::ffi::code::ERR_PANIC,
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn agent_plugin_shutdown(handle: *mut std::ffi::c_void) {
            if handle.is_null() {
                return;
            }
            // SAFETY: handle 只能来自 init 的 into_raw，shutdown 后宿主不再使用
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                drop(unsafe { Box::from_raw(handle as *mut Box<dyn $crate::plugin_side::PluginState>) });
            }));
        }

        #[unsafe(no_mangle)]
        #[allow(clippy::not_unsafe_ptr_arg_deref)]
        pub extern "C" fn agent_plugin_free_buffer(ptr: *mut u8, len: usize) {
            // SAFETY: 宿主只会把本插件 write_out 交出的缓冲送回来
            unsafe { $crate::plugin_side::free_allocated(ptr, len) };
        }
    };
}

#[cfg(test)]
mod tests;
