//! 宿主能力表：静态 `HostVtable` 与 C 回调实现（DESIGN.md §5.1）。
//!
//! 指针必须是 'static（init 时把引用交付给插件，进程内长生不死），
//! 因此用静态实例。回调是 extern "C"：panic 到边界即 abort，所以每个
//! 回调体整体 catch_unwind（纪律 3 的宿主侧）。
//!
//! `kv_get` 阶段三返回 `ERR_UNSUPPORTED`：它需要"宿主分配、插件释放"
//! 的缓冲区所有权协议——_ABI 已按 §15.1 定死签名，等真正需要 kv 的
//! 插件落地时在 vtable **末尾**追加 `kv_free` 再启用（追加安全）。
//!
//! `notify`（⑭，v2.29 末尾追加槽）默认 `ERR_UNSUPPORTED`：桌面投递能力
//! 归组装点——组装点经 [`set_notify_hook`] 注入闭包（plugin-host 不依赖
//! Tauri）；未注入（headless kernel-bin）时插件收到 UNSUPPORTED 如实报
//! 给模型。

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};

use plugin_api::ffi::{HostVtable, code, log_level};

/// 进程级 KV 存储（kv_put 的真实后端；kv_get 待所有权协议后开放）。
static KV: LazyLock<Mutex<HashMap<Vec<u8>, Vec<u8>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// ⑭ 桌面通知后端（组装点注入；None = 无桌面环境）。
type NotifyHook = Arc<dyn Fn(&str, &str) + Send + Sync>;
static NOTIFY_HOOK: Mutex<Option<NotifyHook>> = Mutex::new(None);

/// 组装点注入桌面通知实现（幂等覆盖；须在插件 init 前调用）。
pub fn set_notify_hook(hook: NotifyHook) {
    *NOTIFY_HOOK.lock().expect("notify hook lock poisoned") = Some(hook);
}

static HOST_VTABLE: HostVtable = HostVtable {
    log: host_log,
    kv_get: host_kv_get,
    kv_put: host_kv_put,
    notify: host_notify,
};

/// init 交付给插件的宿主能力表（'static，插件侧可长期持有）。
pub(crate) fn host_vtable() -> &'static HostVtable {
    &HOST_VTABLE
}

extern "C" fn host_log(level: u32, ptr: *const u8, len: usize) {
    let _ = std::panic::catch_unwind(|| {
        if ptr.is_null() || len == 0 {
            return;
        }
        // SAFETY: 调用契约：插件保证 (ptr, len) 指向一段有效日志字节；
        // 恶意/错误输入最多产生无效 UTF-8，用占位文本兜底
        let msg = unsafe { std::slice::from_raw_parts(ptr, len) };
        let msg = std::str::from_utf8(msg).unwrap_or("<plugin log: invalid utf8>");
        match level {
            log_level::ERROR => tracing::error!(target: "plugin", "{msg}"),
            log_level::WARN => tracing::warn!(target: "plugin", "{msg}"),
            log_level::DEBUG => tracing::debug!(target: "plugin", "{msg}"),
            _ => tracing::info!(target: "plugin", "{msg}"),
        }
    });
}

extern "C" fn host_kv_get(
    _k: *const u8,
    _klen: usize,
    _out: *mut *mut u8,
    _olen: *mut usize,
) -> i32 {
    // 见模块注释：缓冲区所有权协议未定，能力先占位不开放。
    code::ERR_UNSUPPORTED
}

extern "C" fn host_kv_put(k: *const u8, klen: usize, v: *const u8, vlen: usize) -> i32 {
    std::panic::catch_unwind(|| {
        if k.is_null() || v.is_null() {
            return code::ERR_BAD_ARGS;
        }
        // SAFETY: 调用契约：插件保证两段指针在调用期间有效
        let (key, val) = unsafe {
            (
                std::slice::from_raw_parts(k, klen),
                std::slice::from_raw_parts(v, vlen),
            )
        };
        KV.lock()
            .expect("kv lock poisoned")
            .insert(key.to_vec(), val.to_vec());
        code::OK
    })
    .unwrap_or(code::ERR_PANIC)
}

extern "C" fn host_notify(
    title: *const u8,
    title_len: usize,
    body: *const u8,
    body_len: usize,
) -> i32 {
    std::panic::catch_unwind(|| {
        if title.is_null() || body.is_null() {
            return code::ERR_BAD_ARGS;
        }
        // SAFETY: 调用契约：插件保证两段指针在调用期间有效；无效 UTF-8
        // 由 lossy 兜底（通知文案不是契约数据）
        let (title, body) = unsafe {
            (
                std::slice::from_raw_parts(title, title_len),
                std::slice::from_raw_parts(body, body_len),
            )
        };
        let title = String::from_utf8_lossy(title);
        let body = String::from_utf8_lossy(body);
        let hook = NOTIFY_HOOK.lock().expect("notify hook lock poisoned");
        match hook.as_ref() {
            Some(h) => {
                h(&title, &body);
                code::OK
            }
            // 无桌面环境（headless）：如实告知插件，由其回给模型
            None => code::ERR_UNSUPPORTED,
        }
    })
    .unwrap_or(code::ERR_PANIC)
}

#[cfg(test)]
mod tests;
