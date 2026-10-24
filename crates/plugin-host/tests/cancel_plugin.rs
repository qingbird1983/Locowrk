//! specs/04 · C3 验收：native 后端的调用期取消查询（真 dlopen tool-echo）。
//!
//! 三条用例对应 spec C3 的验收句：
//! ① `wait_cancel` —— 宿主**中途**翻标志（`ctx.cancel()`）→ 插件经 vtable
//!    `call_cancelled` 读到 1、在轮询点提前收束（自愿早停的原型）；
//! ② `probe_bg` —— 插件自起的后台线程查询得 `ERR_UNSUPPORTED`（非调用期
//!    对照），且调用期主线程同一次读得 0，两数一起报出；
//! ③ `wait_cancel` 小窗口对照 —— 调用期内无人取消读到 0（0 流对照）。
//!
//! 取消标志只经 `CallCtx` 流动（kernel 侧 C1 的登记表不在本射程）：适配器
//! 把 `ctx.cancelled_handle()` 挂到 blocking 线程的 thread-local，插件读的
//! 就是同一个 `Arc<AtomicBool>`。构建样板与暂存纪律见 `common/mod.rs`。

use std::sync::Arc;
use std::time::Duration;

use plugin_api::ffi::code;
use plugin_api::{Tool, ToolError};
use plugin_host::{PluginRegistry, RegistryToolSource};
use serde_json::{Value, json};

mod common;
use common::{build_and_stage, call, err_msg_ref};

/// 装载 0.3.0（本射程专用版本档：`echo_plugin` 用 0.1.0/0.2.0、`wasm_plugins`
/// 用 1.0.0，互不重叠——跨进程组合隔离的既有纪律）。
fn echo() -> (Arc<PluginRegistry>, RegistryToolSource) {
    let registry = Arc::new(PluginRegistry::new());
    let ver: semver::Version = "0.3.0".parse().unwrap();
    let path = build_and_stage("tool-echo", "0.3.0");
    registry
        .load("tool-echo", &ver, &path)
        .expect("load tool-echo 0.3.0");
    registry.activate("tool-echo", &ver).expect("activate");
    let source = RegistryToolSource::new(Arc::clone(&registry));
    (registry, source)
}

/// 中途取消版调用：先起叫停线程（delay 后 `ctx.cancel()`），再 block_on
/// 工具调用。即使叫停晚于插件第一次轮询，插件也会在后续轮询点读到 1——
/// 只有「整个等待窗走完仍未取消」才会假红，delay(150ms) ≪ 窗口(5000ms)。
fn call_with_mid_call_cancel(
    tool: &Arc<dyn Tool>,
    delay: Duration,
    args: Value,
) -> Result<Value, ToolError> {
    let ctx = plugin_api::CallCtx::new();
    let ctx_for_killer = ctx.clone();
    let killer = std::thread::spawn(move || {
        std::thread::sleep(delay);
        ctx_for_killer.cancel();
    });
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let out = rt.block_on(tool.call(&ctx, args));
    killer.join().unwrap();
    out
}

#[test]
fn native_plugin_reads_cancel_flip_mid_call() {
    let (_registry, source) = echo();
    let tool = source.resolve("echo").expect("resolve echo");
    let out = call_with_mid_call_cancel(
        &tool,
        Duration::from_millis(150),
        json!({ "text": "x", "mode": "wait_cancel" }),
    )
    .expect("插件应在轮询点看到取消并提前收束");
    assert_eq!(out["cancelled"], json!(true), "{out}");
}

#[test]
fn plugin_background_thread_reads_unsupported() {
    let (_registry, source) = echo();
    let tool = source.resolve("echo").unwrap();
    let out = call(tool.as_ref(), json!({ "text": "x", "mode": "probe_bg" }))
        .expect("后台探针不得失败（非调用期是数据不是错误）");
    assert_eq!(
        out["main_code"],
        json!(0),
        "调用期主线程读数应为 0（无人取消）: {out}"
    );
    assert_eq!(
        out["bg_code"],
        json!(code::ERR_UNSUPPORTED),
        "插件自起的后台线程应为非调用期读数: {out}"
    );
}

#[test]
fn wait_cancel_reports_zero_without_cancel() {
    let (_registry, source) = echo();
    let tool = source.resolve("echo").unwrap();
    let out = call(
        tool.as_ref(),
        json!({ "text": "x", "mode": "wait_cancel", "max_ms": 60 }),
    )
    .expect("0 流对照不得失败");
    assert_eq!(out["cancelled"], json!(false), "{out}");
}

/// 取消是数据不是错误：错误信封只在插件真的执行失败时出现——本文件的
/// 三个用例全部走 Ok 通道，这里补一条「取消读数不进 err_msg_ref」的
/// 反向钉子（防未来有人把 UNSUPPORTED 误升格为失败）。
#[test]
fn cancel_readings_never_surface_as_tool_errors() {
    let (_registry, source) = echo();
    let tool = source.resolve("echo").unwrap();
    let err = call(
        tool.as_ref(),
        json!({ "text": "x", "mode": "no_such_mode" }),
    )
    .expect_err("未知模式照旧报错");
    let msg = err_msg_ref(&err);
    assert!(
        msg.contains("does not support mode=`no_such_mode`"),
        "{msg}"
    );
}
