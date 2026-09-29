//! WasmBackend：PluginBackend 的沙箱实现。
//!
//! 与 native 的两点分野（§5.8）：
//! - 同实例调用经 `Mutex` 串行——客户机是单线程线性内存，guest 状态
//!   天然不可并发；宿主侧接口（call_blocking 签名）不变
//! - 每次调用前重置 epoch 时间预算：上次超时不污染下一次（预算是
//!   per-call 的，不是 per-instance 的）
//!
//! 缓冲协议（abi.rs 所有权规则）：每次 call 向客户机 `alloc` 一块
//! `[0..8) 输出格 | [8..8+tl) 工具名 | [8+tl..) args`，用完 `free` 两次
//! （调用缓冲 + 输出缓冲）；输出格是两个 i32 cell，guest 往里写
//! (ptr, len)，宿主先校验落在内存界内再拷贝——恶意谎报拿不到越界读。

use std::sync::Mutex;

use plugin_api::ToolError;
use plugin_api::ffi;
use serde_json::Value;
use wasmtime::{Memory, Store, StoreLimits, Trap, TypedFunc};

use super::abi;
use crate::instance::PluginBackend;

/// guest `call` 导出签名：handle + 6 个指针/长度参数 → ffi 返回码。
type GuestCall = TypedFunc<(i32, i32, i32, i32, i32, i32, i32), i32>;

pub(crate) struct WasmBackend {
    store: Mutex<Store<StoreLimits>>,
    memory: Memory,
    alloc: TypedFunc<i32, i32>,
    free: TypedFunc<(i32, i32), ()>,
    call: GuestCall,
    shutdown: TypedFunc<(i32,), ()>,
    /// init 返回的 guest 句柄。
    handle: i32,
    timeout_ms: u64,
    /// epoch 倒数格数（向上取整，≥1）。
    timeout_ticks: u64,
}

impl WasmBackend {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        store: Store<StoreLimits>,
        memory: Memory,
        alloc: TypedFunc<i32, i32>,
        free: TypedFunc<(i32, i32), ()>,
        call: GuestCall,
        shutdown: TypedFunc<(i32,), ()>,
        handle: i32,
    ) -> Self {
        let timeout_ms = abi::timeout_ms();
        let timeout_ticks = timeout_ms.div_ceil(abi::EPOCH_TICK_MS).max(1);
        Self {
            store: Mutex::new(store),
            memory,
            alloc,
            free,
            call,
            shutdown,
            handle,
            timeout_ms,
            timeout_ticks,
        }
    }

    /// 校验并拷出 guest 报告的输出。(0, 0) 表示无输出；负值或越界是
    /// 客户机违约，返回 Err——恶意谎报拿不到越界读。
    fn read_output(
        &self,
        store: &Store<StoreLimits>,
        out_ptr: i32,
        out_len: i32,
    ) -> Result<Option<Vec<u8>>, String> {
        if out_ptr == 0 && out_len == 0 {
            return Ok(None);
        }
        if out_ptr <= 0 || out_len <= 0 {
            return Err(format!(
                "guest reported invalid output pointer/length ({out_ptr}, {out_len})"
            ));
        }
        let (ptr, len) = (out_ptr as usize, out_len as usize);
        let size = self.memory.data_size(store);
        if ptr + len > size {
            return Err(format!(
                "guest reported output outside linear memory (ptr={out_ptr}, len={out_len}, memory={size} bytes)"
            ));
        }
        let mut buf = vec![0u8; len];
        self.memory
            .read(store, ptr, &mut buf)
            .map_err(|e| e.to_string())?;
        Ok(Some(buf))
    }
}

impl PluginBackend for WasmBackend {
    fn call_blocking(&self, tool: &str, args: &Value) -> Result<Value, ToolError> {
        let fail = |message: String| ToolError::Failed {
            tool: tool.to_string(),
            message,
        };
        let args_bytes =
            serde_json::to_vec(args).map_err(|e| fail(format!("args not serializable: {e}")))?;
        let tool_bytes = tool.as_bytes();
        let tool_len =
            i32::try_from(tool_bytes.len()).map_err(|_| fail("tool name too long".into()))?;
        let args_len =
            i32::try_from(args_bytes.len()).map_err(|_| fail("args too large".into()))?;
        let total = 8i32
            .checked_add(tool_len)
            .and_then(|v| v.checked_add(args_len))
            .ok_or_else(|| fail("args too large".into()))?;

        let mut store = self.store.lock().unwrap_or_else(|p| p.into_inner());

        // 时间预算按次调用重置（覆盖 alloc/call/free 整个窗口；上次
        // 耗尽不污染下一次）
        store.set_epoch_deadline(self.timeout_ticks);

        // 单块调用缓冲：输出格 + 工具名 + args
        let base = self
            .alloc
            .call(&mut *store, total)
            .map_err(|e| fail(format!("guest alloc: {e}")))?;
        if base < 0 {
            return Err(fail("guest alloc returned negative pointer".into()));
        }
        let mut write = || -> Result<(), String> {
            self.memory
                .write(&mut *store, base as usize, &[0u8; 8])
                .map_err(|e| e.to_string())?;
            self.memory
                .write(&mut *store, base as usize + 8, tool_bytes)
                .map_err(|e| e.to_string())?;
            self.memory
                .write(
                    &mut *store,
                    base as usize + 8 + tool_bytes.len(),
                    &args_bytes,
                )
                .map_err(|e| e.to_string())
        };
        if let Err(e) = write() {
            let _ = self.free.call(&mut *store, (base, total));
            return Err(fail(format!("write guest memory: {e}")));
        }

        // 时间预算已在本函数入口设置；call 前不再重置
        let code = match self.call.call(
            &mut *store,
            (
                self.handle,
                base + 8,
                tool_len,
                base + 8 + tool_len,
                args_len,
                base,
                base + 4,
            ),
        ) {
            Ok(code) => code,
            Err(e) => {
                let _ = self.free.call(&mut *store, (base, total));
                return Err(fail(call_error(e, self.timeout_ms)));
            }
        };

        let mut cells = [0u8; 8];
        let cells_ok = self.memory.read(&*store, base as usize, &mut cells).is_ok();
        let (out_ptr, out_len) = if cells_ok {
            (
                i32::from_le_bytes(cells[0..4].try_into().expect("4 bytes")),
                i32::from_le_bytes(cells[4..8].try_into().expect("4 bytes")),
            )
        } else {
            (0, 0)
        };
        let payload = match self.read_output(&store, out_ptr, out_len) {
            Ok(p) => p,
            Err(msg) => {
                let _ = self.free.call(&mut *store, (base, total));
                return Err(fail(msg));
            }
        };

        // 归还两块缓冲（尽力而为：guest free 违约不掩盖主结果）
        let _ = self.free.call(&mut *store, (base, total));
        if payload.is_some() {
            let _ = self.free.call(&mut *store, (out_ptr, out_len));
        }

        let Some(bytes) = payload else {
            let message = match code {
                ffi::code::ERR_PANIC => "plugin panicked (caught inside plugin)".to_string(),
                other => format!("plugin returned code {other} with no output"),
            };
            return Err(fail(message));
        };
        let value: Value = serde_json::from_slice(&bytes)
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

/// trap → 错误消息：epoch 耗尽单独成句（用户可懂），其余透传 trap
/// （`{e:#}` 带 anyhow 链，"unreachable" 等根因在链尾）。
fn call_error(e: wasmtime::Error, timeout_ms: u64) -> String {
    if e.downcast_ref::<Trap>() == Some(&Trap::Interrupt) {
        format!("wasm plugin exceeded its time budget ({timeout_ms} ms)")
    } else {
        format!("wasm trap: {e:#}")
    }
}

impl Drop for WasmBackend {
    fn drop(&mut self) {
        // best-effort guest shutdown：trap 静默（沙箱内无宿主资源可泄）
        let mut store = self.store.lock().unwrap_or_else(|p| p.into_inner());
        let _ = self.shutdown.call(&mut *store, (self.handle,));
    }
}
