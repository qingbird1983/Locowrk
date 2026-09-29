//! 插件运行实例：dlopen 句柄 + 导出符号 + init 句柄的持有者（§5.5）。
//!
//! 生命周期纪律：
//! - Drop = shutdown 句柄 → Library drop（dlclose）。这是**全进程唯一**
//!   的 dlclose 路径：registry 只把 Arc 挪进 graveyard（永不 drop），
//!   因此正常运转时 Drop 不可达；可达的只有"加载即失败"——init 从未
//!   执行、无代码指针流出，句柄立即释放（§5.6 纪律一的唯一豁免，附录 C-3）。
//! - 并发调用契约：同一实例可被多线程并发 call（FFI 符号是纯函数指针，
//!   无共享可变状态）；插件内部必须自行保证线程安全（echo 的状态不可变）。
//! - 宿主侧每次跨边界调用都 catch_unwind（纪律 3：双保险的宿主一半）。

use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use libloading::Library;
use plugin_api::ffi::{CallFn, FreeBufFn, ShutdownFn};
use plugin_api::manifest::PluginManifest;
use plugin_api::{ToolError, ToolSpec};
use serde_json::Value;
use tokio::sync::Semaphore;

/// 一个已加载插件版本。共享形态固定为 `Arc<PluginInstance>`。
#[derive(Debug)]
pub struct PluginInstance {
    manifest: PluginManifest,
    version: semver::Version,
    call: CallFn,
    shutdown: ShutdownFn,
    free: FreeBufFn,
    /// init 返回的插件自管句柄，shutdown 后失效。
    handle: *mut c_void,
    /// 声明字段序：Drop 先执行 shutdown（显式写在 drop 里），字段随后
    /// 逆析构，`lib` 最后释放代码段——顺序由 Drop impl 保证。
    _lib: Library,
    /// 同实例的并发调用上限（§5.5：适配器持有 permit，实例共享限额）。
    sem: Arc<Semaphore>,
    /// 加载来源（dll 所在路径）。其父目录是 manifest.json 与图标的
    /// 定位基础（packaging.md §3.1：每个版本目录自包含）。
    path: PathBuf,
}

impl PluginInstance {
    /// loader 专用构造：所有字段已在加载路径上校验完毕。
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        manifest: PluginManifest,
        version: semver::Version,
        call: CallFn,
        shutdown: ShutdownFn,
        free: FreeBufFn,
        handle: *mut c_void,
        lib: Library,
        path: PathBuf,
    ) -> Self {
        Self {
            manifest,
            version,
            call,
            shutdown,
            free,
            handle,
            _lib: lib,
            sem: Arc::new(Semaphore::new(16)),
            path,
        }
    }

    pub fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    pub fn version(&self) -> &semver::Version {
        &self.version
    }

    /// 加载来源路径（dll 所在位置；父目录即版本目录）。
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// manifest 声明的工具规格（名字查找，§5.2）。
    pub fn spec_of(&self, tool: &str) -> Option<ToolSpec> {
        self.manifest.tools.iter().find(|t| t.name == tool).cloned()
    }

    pub(crate) fn semaphore(&self) -> Arc<Semaphore> {
        Arc::clone(&self.sem)
    }

    /// 同步 FFI 调用（阻塞；调用方必须放在 `spawn_blocking` 内，§5.6 纪律三）。
    /// 返回码非零时尽力从输出信封取错误消息。
    pub(crate) fn call_blocking(&self, tool: &str, args: &Value) -> Result<Value, ToolError> {
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
                plugin_api::ffi::code::ERR_PANIC => {
                    "plugin panicked (caught inside plugin)".to_string()
                }
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
            plugin_api::ffi::code::OK => Ok(value),
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

// SAFETY: Library 是 Send+Sync；符号为纯函数指针；handle 由本实例独占
// （只在 call/shutdown 中使用）。并发安全性契约见模块注释。
unsafe impl Send for PluginInstance {}
unsafe impl Sync for PluginInstance {}

impl Drop for PluginInstance {
    fn drop(&mut self) {
        let handle = self.handle;
        let shutdown = self.shutdown;
        // SAFETY: handle 来自 init，Drop 后不再被任何路径使用
        let _ = catch_unwind(AssertUnwindSafe(|| shutdown(handle)));
        // _lib 字段随后析构 → dlclose（"加载即失败"豁免路径）
    }
}
