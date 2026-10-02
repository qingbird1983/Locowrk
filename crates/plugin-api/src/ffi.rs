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
    /// 调用方给的缓冲太小（`session_context` ㉘ 用；调用方换更大缓冲重试）。
    pub const ERR_BUF_TOO_SMALL: i32 = -7;
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
///
/// 追加史：⑭（v2.29）末尾追加 `notify` 桌面通知槽——append-only 不破
/// `PLUGIN_ABI_VERSION`。旧插件只读前缀字段（repr(C) 布局前缀兼容）；
/// 新插件跑旧宿主读越界属版本错配（本仓插件与宿主同源发布）。
/// ㉘（v2.55）末尾追加 `session_context` 调用期会话上下文槽——FFI 边界
/// 不传 CallCtx（DESIGN §6.3 undo 契约同款约束），插件要感知"本次调用
/// 属于哪个会话/项目"（记忆按项目隔离）只能反向查询宿主。**调用期**
/// 语义：宿主在执行本次 FFI call 的线程上预置上下文，插件在该次 call
/// 执行期间查询有效；插件自起的后台线程 / 非调用期查询得
/// `ERR_UNSUPPORTED`（宿主未注入解析器的 headless 宿主同码——插件
/// 应回落全局语义，不得失败）。
#[repr(C)]
pub struct HostVtable {
    pub log: extern "C" fn(level: u32, ptr: *const u8, len: usize),
    pub kv_get:
        extern "C" fn(k: *const u8, klen: usize, out: *mut *mut u8, olen: *mut usize) -> i32,
    pub kv_put: extern "C" fn(k: *const u8, klen: usize, v: *const u8, vlen: usize) -> i32,
    /// 桌面系统通知（⑭）：title/body 都是 UTF-8 字节串，宿主投递系统级
    /// 通知后返回 `code::OK`；无桌面环境（headless 宿主）返回
    /// `code::ERR_UNSUPPORTED`，插件把失败如实回给模型，不得静默。
    pub notify:
        extern "C" fn(title: *const u8, title_len: usize, body: *const u8, body_len: usize) -> i32,
    /// 调用期会话上下文（㉘）：把当前调用的上下文 JSON 写进**调用方
    /// （插件）提供的缓冲**——`{"session_id": str|null, "workspace":
    /// str|null}`（workspace = 会话关联的项目根，未归属为 null）。返回
    /// 写入字节数（`code::OK`）；缓冲不足返回 `code::ERR_BUF_TOO_SMALL`；
    /// 非调用期 / 宿主无解析器返回 `code::ERR_UNSUPPORTED`。**不分配、
    /// 不需要 free**——缓冲归插件，规避 kv_get 卡住的跨分配器所有权问题。
    pub session_context: extern "C" fn(buf: *mut u8, buf_len: usize) -> i32,
}

impl HostVtable {
    /// 插件侧便捷封装：把 UTF-8 字符串经 vtable 打日志。
    ///
    /// # Safety
    /// `self` 必须是 init 时宿主交付的合法指针。
    pub unsafe fn log_str(&self, level: u32, msg: &str) {
        (self.log)(level, msg.as_ptr(), msg.len());
    }

    /// 插件侧便捷封装：经 vtable 发桌面通知（⑭）。
    ///
    /// # Safety
    /// `self` 必须是 init 时宿主交付的合法指针。
    pub unsafe fn notify_str(&self, title: &str, body: &str) -> i32 {
        (self.notify)(title.as_ptr(), title.len(), body.as_ptr(), body.len())
    }

    /// 插件侧便捷封装：查询调用期会话上下文（㉘）。`Ok(n)` = JSON 字节
    /// 数；`Err(code)` = UNSUPPORTED（非调用期 / headless——调用方按全局
    /// 语义回落）或 BUF_TOO_SMALL（换更大缓冲）。
    ///
    /// # Safety
    /// `self` 必须是 init 时宿主交付的合法指针。
    pub unsafe fn session_context(&self, buf: &mut [u8]) -> Result<usize, i32> {
        let n = (self.session_context)(buf.as_mut_ptr(), buf.len());
        if n >= 0 { Ok(n as usize) } else { Err(n) }
    }
}

/// 调用期会话上下文载荷（㉘，`session_context` 槽跨边界以 JSON 传输）。
/// `workspace` = 会话关联的项目根（§2.3 工作区），未归属为 `None`；
/// 插件据此把数据按项目隔离（memory-sqlite 首个消费方）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallContext {
    pub session_id: Option<String>,
    pub workspace: Option<String>,
}

impl CallContext {
    pub fn to_json(&self) -> String {
        crate::serde_json::json!({
            "session_id": self.session_id,
            "workspace": self.workspace,
        })
        .to_string()
    }

    /// 容错解析：坏 JSON / 缺字段返回 `None`（调用方按全局语义回落）。
    pub fn from_json(bytes: &[u8]) -> Option<Self> {
        let v: crate::serde_json::Value = crate::serde_json::from_slice(bytes).ok()?;
        let opt_str = |k: &str| {
            v.get(k)
                .and_then(crate::serde_json::Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
        };
        Some(Self {
            session_id: opt_str("session_id"),
            workspace: opt_str("workspace"),
        })
    }
}

#[cfg(test)]
mod tests;
