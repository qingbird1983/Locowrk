//! plugin-host 集成测试共用夹具（《AUDIT-FIX-PLAN-v1》批 F·F8 + F11b）。
//!
//! 收敛前这五个集成测试文件（`echo_plugin` / `fs_fetch_plugins` / `tools_batch2`
//! / `tools_batch3` / `wasm_plugins`）**各抄一份同构样板**：`build_lock` /
//! `workspace_dir` / `artifact_name` / `build_and_stage` / `call` / `load_activate`
//! 各 ×5，`env_lock` / `temp_root` / `err_msg` 各 ×2 —— `tools_batch2.rs` 与
//! `tools_batch3.rs` 的夹具区除一个前缀字符串外逐字相同。同语义实现分家正是
//! 「改一处漏一处」的产地（v2.133 ENV_LOCK 分裂、v2.135 gc 序错同源），现在照
//! `kernel-agent/tests/common/mod.rs` 的先例收成一份，各文件以 `mod common;` 引入。
//!
//! 每个测试二进制只会用到其中一部分，未用项是常态 → 整模块 `#![allow(dead_code)]`
//! （同款先例）。
//!
//! **F11b：暂存目录带 pid 分层**（`target/locowrk-test/{pid}/{plugin}/{version}/`）。
//! 原来所有测试二进制共用 `target/locowrk-test/{plugin}/{version}/`，`stage()` 的
//! `create_dir_all` + `copy` 不是原子的，两个进程写同一路径时另一半可能读到写了一半的
//! dll（Windows 还会报「文件正在使用」——`wasm_plugins.rs` 里那条 tmp 注释记的就是这个
//! 症状）。加一层 pid 后各进程写各的目录。
//!
//! ⚠ 诚实边界：按**当前**的调用表，两个二进制请求同一 `(plugin, version)` 的情况其实
//! 不存在（tool-echo 被 `echo_plugin` 用 0.1.0/0.2.0、被 `wasm_plugins` 用 1.0.0；
//! tool-fs 被 `fs_fetch` 用 1.2.1、被 `tools_batch2` 用 0.1.5……逐条查过），所以这一层
//! 是**结构性隔离（防今后新增重叠组合）**，不是"修好了历批记的偶发红"——那条的真实
//! 机制本批未确证，见下面的 ⚠ 段。
//!
//! ⚠ **本层没有解决、也没有确证的一条**：`cargo build` 的输出固定在共享的
//! `target/debug/`，而 `LOCOWRK_PLUGIN_VERSION` 是编译期注入——两个二进制并发构建同一
//! 插件的**不同版本**时（现存的组合：`fs_fetch` 要 tool-fs 1.2.1 与 `tools_batch2` 要
//! tool-fs 0.1.5；`echo_plugin` 的 tool-echo 0.1.0/0.2.0 与 `wasm_plugins` 的 1.0.0），
//! 理论上后构建的一方会覆盖 `target/debug` 里的产物，前一方「构建返回 → 复制」之间的
//! 窗口里可能复制到对方的字节。**但本批定向压测没能复现**：`fs_fetch_plugins` 与
//! `tools_batch2` 两个测试二进制并行 4 轮（8 个结果）全绿，另 `cargo test -p
//! plugin-host` 连续 8 轮全绿；批 F-2b 期间抓到过一次红（fs_fetch 4 failed / 1 passed，
//! 0.8s 内失败 = 不像 5s 网络超时）此后再未复现，**机制未确证**。因此这里不凭猜测
//! 造跨进程锁（那要单独定口径：新依赖 fs-lock 类，或自研锁文件 + 陈旧回收），只把
//! 失败报文补成 `(plugin_id, version, 暂存路径)` 三要素——见 [`load_activate`]，
//! 下次再红能直接定性。本层 `build_lock()` 仍只串行化**同一二进制内**的构建。
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use plugin_api::{Tool, ToolError};
use plugin_host::{PluginRegistry, RegistryToolSource};
use semver::Version;
use serde_json::Value;

/// 串行化**同一测试二进制内**的构建（`LOCOWRK_PLUGIN_VERSION` 变更会重编同一产物路径）。
/// 跨二进制的互斥不在本层，见模块头注 ⚠。
pub fn build_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// 进程级 env 串行锁：动 `std::env::set_var` 的用例必须互斥（同一二进制内）。
pub fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Cargo 工作区根（`crates/plugin-host/../..`）。
pub fn workspace_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root")
}

/// 本轮运行的暂存根：`target/locowrk-test/{pid}`（F11b：并行进程各写各的目录）。
/// **访问即确保存在**——调用方有的会直接往这里写临时工件（`stage_ping_guest` 的
/// tmp wasm），目录不存在时 `fs::write` 会 NotFound（本函数首版就漏了 create，
/// 由 wasm_plugins 两个测试当场抓红）。
pub fn run_root() -> PathBuf {
    let d = workspace_dir()
        .join("target/locowrk-test")
        .join(format!("{}", std::process::id()));
    std::fs::create_dir_all(&d).expect("run_root: 暂存根创建失败");
    d
}

/// 插件产物在本平台的文件名（`tool-fs` → `tool_fs.dll` / `libtool_fs.so` /
/// `libtool_fs.dylib`）。
pub fn artifact_name(plugin_id: &str) -> String {
    let stem = plugin_id.replace('-', "_");
    if cfg!(windows) {
        format!("{stem}.dll")
    } else if cfg!(target_os = "macos") {
        format!("lib{stem}.dylib")
    } else {
        format!("lib{stem}.so")
    }
}

/// 把任意工件落成版本目录布局（`{run_root}/{plugin_id}/{version}/{file_name}`）。
/// wasm 客户机不走 cargo build，直接给字节时也走这里（packaging.md §3.1：
/// manifest.json 与工件同目录）。
pub fn stage(src: &Path, plugin_id: &str, version: &str, file_name: &str) -> PathBuf {
    let dst_dir = run_root().join(plugin_id).join(version);
    std::fs::create_dir_all(&dst_dir).unwrap();
    let dst = dst_dir.join(file_name);
    std::fs::copy(src, &dst).unwrap();
    dst
}

/// 以指定版本构建插件（env `LOCOWRK_PLUGIN_VERSION` 注入）并把产物复制进本轮暂存目录。
pub fn build_and_stage(plugin_id: &str, version: &str) -> PathBuf {
    let _guard = build_lock().lock().unwrap();
    let status = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
        .args(["build", "-p", plugin_id, "--quiet"])
        .env("LOCOWRK_PLUGIN_VERSION", version)
        .status()
        .expect("spawn cargo");
    assert!(
        status.success(),
        "cargo build -p {plugin_id} ({version}) failed"
    );

    let src = workspace_dir()
        .join("target/debug")
        .join(artifact_name(plugin_id));
    assert!(src.exists(), "artifact missing at {}", src.display());
    stage(&src, plugin_id, version, &artifact_name(plugin_id))
}

/// 装载并激活到**已有**注册表（batch 系列的用法）。
///
/// 失败报文带上 `(plugin_id, version, 暂存路径)`：批 F-2b 复现期间抓到过一次偶发
/// 红（`fs_fetch_plugins` 4 failed / 1 passed，0.8s 内失败）但没能再复现，机制未
/// 确证。原报文只有一个光秃秃的 "load"，红时无从判断是版本覆盖还是端口/环境问题，
/// 故这里把三要素打进 panic 文本——下次再红就能直接定性。
pub fn load_activate(plugin_id: &str, version: &str, registry: &Arc<PluginRegistry>) {
    let ver: Version = version.parse().unwrap();
    let path = build_and_stage(plugin_id, version);
    registry
        .load(plugin_id, &ver, &path)
        .unwrap_or_else(|e| panic!("load {plugin_id} {version} @ {}: {e}", path.display()));
    registry
        .activate(plugin_id, &ver)
        .unwrap_or_else(|e| panic!("activate {plugin_id} {version} @ {}: {e}", path.display()));
}

/// 新建注册表 + 装载激活 + 返回 `(registry, tool_source)`（`fs_fetch_plugins` 的用法）。
///
/// `registry` 必须在 `source` 存活期间不被丢弃，故两个句柄一起返回（原五份本地实现
/// 里只有这一份返回元组，其余三份是往调用方自备的 registry 上挂）。
pub fn stage_and_load(plugin_id: &str, version: &str) -> (Arc<PluginRegistry>, RegistryToolSource) {
    let registry = Arc::new(PluginRegistry::new());
    load_activate(plugin_id, version, &registry);
    let source = RegistryToolSource::new(Arc::clone(&registry));
    (registry, source)
}

/// 同步调用一个工具（每次建一个 current-thread runtime，与原五份本地实现一致）。
pub fn call(tool: &dyn Tool, args: Value) -> Result<Value, ToolError> {
    let ctx = plugin_api::CallCtx::new();
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(tool.call(&ctx, args))
}

/// 工具错误 → 报文文本（`ToolError` 的 Display 对 Failed 会带上结构，断言只想要正文）。
pub fn err_msg(e: ToolError) -> String {
    err_msg_ref(&e)
}

/// [`err_msg`] 的借用形态（`echo_plugin.rs` 收敛前的 `echo_error(&err)` 用的就是这个）。
pub fn err_msg_ref(e: &ToolError) -> String {
    match e {
        ToolError::Failed { message, .. } => message.clone(),
        other => other.to_string(),
    }
}

/// 唯一临时根（本 crate 与 desktop 的 `testutil` 不互通：后者是 `#[cfg(test)]`
/// 私有模块，跨 crate 不可见，故这里保有一份同源形状的极小实现）。
/// 形状 = `locowrk-{tag}-{pid}-{纳秒}` 且不预建，与 `tools_batch2/3` 收敛前的
/// 行为逐字一致（调用方自己 `create_dir_all`）。
pub fn temp_root(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("locowrk-{tag}-{}-{nanos}", std::process::id()))
}
