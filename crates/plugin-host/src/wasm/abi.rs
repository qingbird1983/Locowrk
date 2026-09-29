//! WASM 插件 ABI（DESIGN.md §5.8 落地）：客户机导出、宿主导入与
//! 沙箱预算。manifest 仍是 C ABI 那份 JSON（`abi_version` 同值同义：
//! 工具信封契约版本），变的只是传输层。

// ---- 客户机导出（wasm32，指针 = i32 线性内存偏移）----
//   memory                  导出的线性内存
//   describe() -> ptr       NUL 结尾 UTF-8 JSON manifest（静态数据，
//                           不经由 alloc，宿主永不 free）
//   alloc(len) -> ptr       为宿主划一块缓冲区；每次 call 调用一次，
//                           布局 [0..8)=输出格 two i32 cell、
//                           [8..8+tl)=工具名、[8+tl..)=args JSON
//   free(ptr, len)          归还宿主用过的缓冲区（调用缓冲与输出缓冲）
//   init() -> handle        客户机自选句柄；宿主能力经导入注入，
//                           不传 vtable 指针（与 C ABI 的签名分野）
//   call(handle, tool_ptr, tool_len, args_ptr, args_len,
//        out_ptr_cell, out_len_cell) -> code   返回码同 ffi::code
//   shutdown(handle)
//
// ---- 宿主导入（命名空间 locowrk，能力面是 C ABI vtable 的沙箱投影）----
//   log(level, ptr, len)    客户机内存日志 → tracing target "plugin"
//
// 未导出的 vtable 能力（kv_*）不在沙箱面里：需要时经"导入追加"放开
// （ABI 冻结纪律在导入命名空间同样成立：只加不减不改签名）。
//
// ---- 所有权规则 ----
//   宿主经 alloc 拿到的每一块内存，用完必须恰好 free 一次（调用缓冲、
//   输出缓冲各自一次）；describe 返回的静态指针永不 free。宿主从不持有
//   客户机内存的引用跨过 free——先拷贝再释放，与 C ABI 同规。
//
// ---- 沙箱预算（无 WASI：无文件/网络/env/时钟面）----
//   内存上限、epoch 时间预算见下列常量；超限即 trap，映射为
//   LoadError / ToolError，绝不 panic 宿主。

/// 宿主导入命名空间。
pub(crate) const HOST_IMPORT_MODULE: &str = "locowrk";

/// 每次 call 的时间预算（缺省 10s；env 覆写，钳到 [1, MAX]）。
pub(crate) const TIMEOUT_DEFAULT_MS: u64 = 10_000;
pub(crate) const TIMEOUT_MAX_MS: u64 = 600_000;
pub(crate) const TIMEOUT_ENV: &str = "LOCOWRK_WASM_TIMEOUT_MS";

/// 引擎 epoch 心跳周期：epoch 每 TICK 自增一次，`set_epoch_deadline`
/// 以 TICK 为单位倒数。
pub(crate) const EPOCH_TICK_MS: u64 = 20;

/// 单实例内存上限（Store limiter，恶意客户机撑内存的硬闸）。
pub(crate) const MEM_LIMIT_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MEM_MAX_MEMORIES: u32 = 1;
pub(crate) const TABLE_MAX_ELEMENTS: u32 = 10_000;
pub(crate) const TABLE_MAX_TABLES: u32 = 4;

/// describe JSON 的 NUL 扫描上限（客户机忘写 NUL 时的兜底闸）。
pub(crate) const DESCRIBE_SCAN_MAX: usize = 1024 * 1024;

/// 单条日志字节上限（宿主导入侧防线，超出截断）。
pub(crate) const LOG_MAX_BYTES: usize = 8 * 1024;

/// 每实例时间预算：读 env（读不到/非法用缺省），钳到 [1, TIMEOUT_MAX_MS]。
pub(crate) fn timeout_ms() -> u64 {
    let raw = std::env::var(TIMEOUT_ENV).ok().and_then(|v| v.parse().ok());
    raw.unwrap_or(TIMEOUT_DEFAULT_MS).clamp(1, TIMEOUT_MAX_MS)
}
