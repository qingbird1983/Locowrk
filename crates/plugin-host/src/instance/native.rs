//! native 后端：dlopen 句柄 + 导出符号 + init 句柄（§5.5，C ABI）。
//!
//! 生命周期纪律：
//! - Drop = **只** shutdown 句柄；模块映像由 `ManuallyDrop<Library>` 永久保留，
//!   所以这条路径上**不会**发生 dlclose（§5.6 纪律一从注释落进类型）。全仓真会
//!   dlclose 的只剩 `loader.rs` 的"加载即失败"：那时 init 从未执行、没有任何代码
//!   指针流出，句柄立即释放是安全的（§5.6 纪律一的唯一豁免，附录 C-3）。
//! - 为什么必须钉死：Linux 实测（本机 WSL，2026-10-17，specs/03 的 T18）——一旦真
//!   dlclose，已经交出去的 dyn 指针所在代码页会被先卸掉，后到的 Drop 跳进空 vtable
//!   槽 ⇒ SIGSEGV。Windows 上 FreeLibrary 数不到零（dll 正被占用）所以看不见这一层。
//! - 并发调用契约：同一实例可被多线程并发 call（FFI 符号是纯函数指针，
//!   无共享可变状态）；插件内部必须自行保证线程安全（echo 的状态不可变）。
//! - 宿主侧每次跨边界调用都 catch_unwind（纪律 3：双保险的宿主一半）。

use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::panic::{AssertUnwindSafe, catch_unwind};

use libloading::Library;
use plugin_api::ffi::{CallFn, FreeBufFn, ShutdownFn};
use plugin_api::{ToolError, ffi};
use serde_json::Value;

use super::PluginBackend;

pub(crate) struct NativeBackend {
    call: CallFn,
    shutdown: ShutdownFn,
    free: FreeBufFn,
    /// init 返回的插件自管句柄。Drop 里取走它并**当场置空**，所以同一个 handle
    /// 被释放两次在类型上不可能发生（双重释放 = 第二次跳进已回收内存的 vtable）。
    handle: *mut c_void,
    /// 声明字段序：Drop 先执行 shutdown，字段随后逆析构；`_lib` 是 ManuallyDrop，
    /// 因此"最后释放代码段"这一步被有意跳过。
    _lib: ManuallyDrop<Library>,
}

impl NativeBackend {
    /// loader 专用构造：符号已在加载路径上解析并完成 init。
    pub(crate) fn new(
        call: CallFn,
        shutdown: ShutdownFn,
        free: FreeBufFn,
        handle: *mut c_void,
        lib: Library,
    ) -> Self {
        Self {
            call,
            shutdown,
            free,
            handle,
            _lib: ManuallyDrop::new(lib),
        }
    }
}

impl PluginBackend for NativeBackend {
    fn call_blocking(&self, tool: &str, args: &Value) -> Result<Value, ToolError> {
        let fail = |message: String| ToolError::Failed {
            tool: tool.to_string(),
            message,
        };
        let args_bytes =
            serde_json::to_vec(args).map_err(|e| fail(format!("args not serializable: {e}")))?;
        let mut out: *mut u8 = std::ptr::null_mut();
        let mut out_len: usize = 0;
        let tool_bytes = tool.as_bytes();
        let code = {
            let call = self.call;
            let handle = self.handle;
            // SAFETY: handle 在 shutdown 前有效；入参切片在调用期间存活
            catch_unwind(AssertUnwindSafe(|| {
                call(
                    handle,
                    tool_bytes.as_ptr(),
                    tool_bytes.len(),
                    args_bytes.as_ptr(),
                    args_bytes.len(),
                    &mut out,
                    &mut out_len,
                )
            }))
            .map_err(|_| fail("plugin panicked across FFI (caught by host)".into()))?
        };
        if out.is_null() || out_len == 0 {
            let message = match code {
                ffi::code::ERR_PANIC => "plugin panicked (caught inside plugin)".to_string(),
                other => format!("plugin returned code {other} with no output"),
            };
            return Err(fail(message));
        }
        // SAFETY: 契约：非零 out_len 时 out 指向插件已写好的 len 字节
        let payload = unsafe { std::slice::from_raw_parts(out, out_len) }.to_vec();
        // 先拷贝再释放：绝不持有插件内存的引用跨过 free
        let free = self.free;
        // SAFETY: (out, out_len) 由插件分配，按 §5.1 契约交还其 free_buffer
        catch_unwind(AssertUnwindSafe(|| free(out, out_len)))
            .map_err(|_| fail("plugin panicked inside free_buffer".into()))?;

        let value: Value = serde_json::from_slice(&payload)
            .map_err(|e| fail(format!("plugin output is not json: {e}")))?;
        match code {
            ffi::code::OK => Ok(value),
            _ => {
                let message = value
                    .get("error")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .unwrap_or_else(|| format!("plugin returned code {code}"));
                Err(fail(message))
            }
        }
    }
}

// SAFETY: Library 是 Send+Sync；符号为纯函数指针；handle 由本后端独占
// （只在 call/shutdown 中使用）。并发安全性契约见模块注释。
unsafe impl Send for NativeBackend {}
unsafe impl Sync for NativeBackend {}

impl Drop for NativeBackend {
    fn drop(&mut self) {
        // 取走 handle 并当场置空：本结构体的 Drop 只会跑一次，而"同一个插件句柄
        // 被 shutdown 两次"是这段生命周期里唯一还能造成双重释放的形态。
        let handle = std::mem::replace(&mut self.handle, std::ptr::null_mut());
        if handle.is_null() {
            return; // init 未成功（加载即失败）：没有插件侧状态可释放
        }
        let shutdown = self.shutdown;
        // SAFETY: handle 来自 init，交给插件侧释放后不再被任何路径使用
        let _ = catch_unwind(AssertUnwindSafe(|| shutdown(handle)));
        // 字段 `_lib` 是 ManuallyDrop ⇒ 到此为止**不**发生 dlclose（§5.6 纪律一）
    }
}
