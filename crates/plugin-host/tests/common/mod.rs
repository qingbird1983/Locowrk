//! plugin-host 集成测试共用夹具（《AUDIT-FIX-PLAN-v1》批 F·F8 + F11b + F11c）。
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
//! **F11b + F11c：暂存根按进程独占，快照目录按调用独占**
//! （`target/locowrk-test/{pid}-{启动纳秒}/{plugin}/{version}/stage-{n}/{artifact}`）。
//! 收敛前所有测试二进制共用 `target/locowrk-test/{plugin}/{version}/`，而 `stage()` 的
//! `create_dir_all` + `copy` 不是原子的、`registry.load()` 的读又发生在锁外——写的一方
//! truncate 的瞬间，读的一方拿到 0 字节 / 半截 dll（Windows 上另一侧直接报 os error 32
//! 「另一个程序正在使用此文件」）。两层各治一半：**进程层**隔不同二进制（批 F-2b），
//! **`stage-{n}` 层**隔同一二进制内的并行用例（批 F-4 = F11c）。
//!
//! **F11c 的三段史（一条老"偶发"是怎么钉死的）**：批 F-2b 只加了进程层，并把当时的
//! 偶发红判为「机制未确证」；批 F-3（v2.153）满跑抓到 `wasm_plugins` 红
//! `unexpected end-of-file (at offset 0x0)`，**而报文里的路径已经带着 pid 分层** ⇒ 病不
//! 在跨进程，在同进程共享写目标，机制就此确证并立为 F11c。本批先用一次性探针把窗口
//! 放大（8 线程 × 200 轮 × 64 KiB 载荷）跑出必红复现 = **脏读 1322 / copy 报错 61 /
//! 总次数 1600**，再落 `stage-{n}`，同一探针转绿（证据见《AUDIT-FIX-PLAN-v1》§5 批 F-4 段）。
//!
//! ⚠ 诚实边界：进程层当年那句「两个二进制请求同一 `(plugin, version)` 的情况并不存在」
//! 仍然成立（tool-echo 被 `echo_plugin` 用 0.1.0/0.2.0、被 `wasm_plugins` 用 1.0.0；
//! tool-fs 被 `fs_fetch` 用 1.2.1、被 `tools_batch2` 用 0.1.5……逐条查过），所以跨进程
//! 那层至今仍是**结构性隔离（防今后新增重叠组合）**；本批真正修掉的是**同进程**那一半。
//!
//! ⚠ **上一条已于 2026-10-24（T42）确证并关账**：全量门禁连红两轮——`echo_plugin`
//! 的 0.1.0 快照读到了 `wasm_plugins`（env=1.0.0）刚写进 `target/debug` 的字节，
//! `VersionMismatch { expected: "0.1.0", manifest: "1.0.0" }`，隔离复跑双双全绿。
//! 修法走了当年预留的「自研锁目录 + 陈旧回收」口径（不引 fs-lock 类依赖）：独占
//! 目录 `target/locowrk-test/build.lock` 的 `create_dir` 在两大平台都是原子判失败，
//! 持锁者留 pid 文件；持有者崩溃留死目录 = 300s 陈旧后由后来者回收重试。
//! [`build_lock`] 现在串行化的是**全部进程**的插件构建，不再只管同一二进制。
//!
//! ⚠ **代价（如实记账）**：`stage-{n}` 让每次调用都留一份完整副本，同一 `(id, version)`
//! 在一轮内不再互相覆盖 ⇒ `target/locowrk-test/` 的增长比改前快（本机 dll 0.6–1.0 MB、
//! 一轮约 18 次 `stage()` 调用）。治理口径与批 F-2a「治脏读不治累积」一致：不在本批
//! 扩大改动面，累积目录由 `cargo clean` 一并清（批 G 射程）。
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};

use plugin_api::{Tool, ToolError};
use plugin_host::{PluginRegistry, RegistryToolSource};
use semver::Version;
use serde_json::Value;

/// 跨进程构建锁的 RAII 守卫：丢弃 = 释放（删独占目录）。
pub struct BuildLockGuard;

impl Drop for BuildLockGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(build_lock_dir());
    }
}

fn build_lock_dir() -> PathBuf {
    workspace_dir().join("target/locowrk-test/build.lock")
}

/// 跨进程构建锁（T42，2026-10-24 落地）：独占目录 = std 里的跨平台互斥原语——
/// `create_dir` 在 NTFS 与 ext4 上对已存在路径都原子判失败。为什么不用文件锁：
/// std 没有文件锁，引 fs-lock 类依赖只为测试基建不值得；为什么不用 `Mutex`：
/// 它只串行化**同一二进制**，而 cargo 并行跑测试二进制（2026-10-24 门禁连红两轮
/// 的根因，机制与实证见 specs/03 **T42** 与本模块头注）。
///
/// 陈旧回收：持锁进程崩溃（含 panic 越过守卫之前被强杀）会留下死目录 ⇒ 后来者
/// 按目录 mtime 判 300s 陈旧即整个删掉重试——300s 远大于一次插件构建（实测 ≤60s），
/// 误回收活锁的代价 = 退回一次旧竞态窗口，比死锁等一整轮好。拿锁本身等 300s，
/// 超时 panic 并指路手删。
pub fn build_lock_acquire() -> BuildLockGuard {
    use std::time::{Duration, Instant};
    const STALE: Duration = Duration::from_secs(300);
    const WAIT: Duration = Duration::from_secs(300);
    let dir = build_lock_dir();
    let deadline = Instant::now() + WAIT;
    loop {
        match std::fs::create_dir(&dir) {
            Ok(()) => {
                let _ = std::fs::write(dir.join("pid"), std::process::id().to_string());
                return BuildLockGuard;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let stale = std::fs::metadata(&dir)
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.elapsed().ok())
                    .is_some_and(|age| age > STALE);
                if stale {
                    // 另一个回收者可能已删：NotFound 静默，下一圈重试。
                    let _ = std::fs::remove_dir_all(&dir);
                    continue;
                }
                if Instant::now() > deadline {
                    panic!(
                        "build lock {} 分钟未拿到（{} 在场且非陈旧）——上一轮测试进程崩了？手删该目录重试",
                        WAIT.as_secs() / 60,
                        dir.display()
                    );
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => panic!("build lock: {e}"),
        }
    }
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

/// 本轮运行的暂存根：`target/locowrk-test/{pid}-{启动纳秒}`。
///
/// 为什么不只带 pid（F-2b 首版只带 pid，本批 F11c 补上纳秒）：**Windows 会回收复用
/// pid**，只带 pid 时下一次运行可能落进上一次留下的同名目录，读到的是**上一轮的残留
/// 产物**——正是批 F-2a 在 desktop 侧 `testutil.rs` 立过的教训（当时 `.undo.bak` 读出
/// 6 条 vs 期望 4 条），口径在此沿用。一个进程一个独占根之后，`stage()` 的调用级序号
/// 才真正闭合「写目标永不共享」。
///
/// **访问即确保存在**——调用方有的会直接往这里写临时工件（`stage_ping_guest` 的
/// tmp wasm），目录不存在时 `fs::write` 会 NotFound（本函数首版就漏了 create，
/// 由 wasm_plugins 两个测试当场抓红）。
pub fn run_root() -> PathBuf {
    static RUN_TOKEN: OnceLock<String> = OnceLock::new();
    let token = RUN_TOKEN.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{}-{nanos}", std::process::id())
    });
    let d = workspace_dir().join("target/locowrk-test").join(token);
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

/// 把任意工件落成**每次调用独占**的快照目录
/// `{run_root}/{plugin_id}/{version}/stage-{n}/{file_name}`（F11c）。
///
/// 为什么必须有 `{n}`：`{pid}` 只隔离**进程**，不隔离同一测试二进制内的**并行用例**。
/// `wasm_plugins.rs` 的两个用例都往同一个 `wasm-ping/0.1.0/wasm_ping.wasm` 做
/// `fs::copy`（truncate + 写），而 `registry.load()` 的读发生在 `build_lock` **之外**
/// → 并发时读到 0 字节 / 半截，报文正是 `unexpected end-of-file (at offset 0x0)`；
/// Windows 上另一侧还会直接报「另一个程序正在使用此文件」（os error 32）。一次性探针
/// 把这条竞争从"满跑偶尔一次"变成"每次必红"：8 线程 × 200 轮、64 KiB 载荷，实测
/// **脏读 1322 / copy 报错 61 / 总次数 1600**（证据见《AUDIT-FIX-PLAN-v1》§5 批 F-4 段）。
///
/// 为什么不能用「复制前先看内容是否一致」或「原子 rename 覆盖」绕过：本仓的纪律是
/// **永不 dlclose**（DESIGN §5 / AGENTS 纪律 2），已装载的 dll 一直被系统映射着，第二个
/// 调用方无论 truncate 还是 rename-over 都会撞文件占用——只有「一人一份」成立。
///
/// `(id, version)` 的**注册语义不变**：registry 只认工件路径，`plugin_dir()` 取其父目录
/// （packaging.md §3.1 的 manifest/图标定位基础因此照常成立，目录层级对它是透明的）。
/// wasm 客户机不走 cargo build，直接给字节时也走这里。
pub fn stage(src: &Path, plugin_id: &str, version: &str, file_name: &str) -> PathBuf {
    static STAGE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = STAGE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dst_dir = run_root()
        .join(plugin_id)
        .join(version)
        .join(format!("stage-{n}"));
    std::fs::create_dir_all(&dst_dir).unwrap();
    let dst = dst_dir.join(file_name);
    std::fs::copy(src, &dst).unwrap();
    dst
}

/// 以指定版本构建插件（env `LOCOWRK_PLUGIN_VERSION` 注入）并把产物复制进本轮暂存目录。
pub fn build_and_stage(plugin_id: &str, version: &str) -> PathBuf {
    let _guard = build_lock_acquire();
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
