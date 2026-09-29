//! WasmLoader：wasmtime 编译 + 受限实例化 + describe/init 握手。
//!
//! 与 DylibLoader 同一条"加载即失败"豁免语义：init 之前任何失败直接
//! 返回错误，实例（Engine 共享、Store 私有）随错误 Drop——wasm 没有
//! dlclose，卸载天然就是释放引用（§5.8 好处三）。
//!
//! 沙箱配置（§5.8 好处一）：
//! - 无 WASI：Linker 只注入 `locowrk.log`，客户机声明任何其他导入
//!   （含 wasi_*）一律实例化失败——能力面由宿主枚举，而不是请求方
//! - Store limiter：内存 64 MiB / 1 块内存 / 表 4 张 × 10000 元素
//! - epoch interruption：进程级心跳线程驱动，预算见 abi.rs

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use plugin_api::PLUGIN_ABI_VERSION;
use plugin_api::ffi::log_level;
use plugin_api::manifest::PluginManifest;
use wasmtime::{
    Caller, Config, Engine, Extern, Linker, Memory, Module, Store, StoreLimits, StoreLimitsBuilder,
    Trap, TypedFunc,
};

use super::abi;
use super::backend::WasmBackend;
use crate::error::LoadError;
use crate::instance::PluginInstance;
use crate::loader::PluginLoader;

pub struct WasmLoader;

impl PluginLoader for WasmLoader {
    fn load(&self, path: &Path) -> Result<Arc<PluginInstance>, LoadError> {
        Ok(Arc::new(Self::load(path)?))
    }
}

impl WasmLoader {
    /// 编译并实例化一个 `.wasm` 插件，完成 describe/manifest/ABI/init 握手。
    pub fn load(path: &Path) -> Result<PluginInstance, LoadError> {
        let engine = engine();
        let bytes = std::fs::read(path).map_err(|e| step(path, "read", e))?;
        let module = Module::from_binary(engine, &bytes).map_err(|e| step(path, "compile", e))?;

        let limits = StoreLimitsBuilder::new()
            .memory_size(abi::MEM_LIMIT_BYTES as usize)
            .memories(abi::MEM_MAX_MEMORIES as usize)
            .table_elements(abi::TABLE_MAX_ELEMENTS as usize)
            .tables(abi::TABLE_MAX_TABLES as usize)
            .build();
        let mut store = Store::new(engine, limits);
        store.limiter(|lim| lim);
        // epoch interruption 下 Store 的默认 deadline 是 0：心跳一自增
        // describe/init 就会 trap。握手与单次调用共用同一时间预算
        // （backend 每次调用前会重置）
        store.set_epoch_deadline(abi::timeout_ms().div_ceil(abi::EPOCH_TICK_MS).max(1));

        let linker = build_linker(engine).map_err(|e| step(path, "linker", e))?;
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| step(path, "instantiate", e))?;

        let memory = instance
            .get_memory(&mut store, "memory")
            .ok_or_else(|| missing(path, "memory"))?;
        let describe = func::<(), i32>(&instance, &mut store, path, "describe")?;
        let alloc = func::<i32, i32>(&instance, &mut store, path, "alloc")?;
        let free = func::<(i32, i32), ()>(&instance, &mut store, path, "free")?;
        let init = func::<(), i32>(&instance, &mut store, path, "init")?;
        let call =
            func::<(i32, i32, i32, i32, i32, i32, i32), i32>(&instance, &mut store, path, "call")?;
        let shutdown = func::<(i32,), ()>(&instance, &mut store, path, "shutdown")?;

        // describe → manifest（静态指针，宿主不 free）
        let ptr = describe
            .call(&mut store, ())
            .map_err(|e| step(path, "describe", e))?;
        let bytes =
            read_nul_terminated(&memory, &store, ptr).map_err(|e| step(path, "describe", e))?;
        let manifest = PluginManifest::from_json(&bytes)?;
        if manifest.abi_version != PLUGIN_ABI_VERSION {
            return Err(LoadError::AbiMismatch {
                expected: PLUGIN_ABI_VERSION,
                found: manifest.abi_version,
            });
        }
        let version =
            semver::Version::parse(&manifest.version).map_err(|_| LoadError::BadVersion {
                manifest: manifest.version.clone(),
            })?;

        let handle = init
            .call(&mut store, ())
            .map_err(|e| step(path, "init", e))?;
        Ok(PluginInstance::new(
            manifest,
            version,
            Box::new(WasmBackend::new(
                store, memory, alloc, free, call, shutdown, handle,
            )),
            path.to_path_buf(),
        ))
    }
}

/// 阶段化错误包装（闭包参数吃不下 `impl Display`，用自由函数）。
fn step(path: &Path, stage: &str, e: impl std::fmt::Display) -> LoadError {
    LoadError::Wasm {
        path: path.display().to_string(),
        message: format!("{stage}: {e:#}"),
    }
}

/// 进程级共享引擎（epoch 心跳只起一个线程；JIT 缓存天然共享）。
fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let mut config = Config::new();
        config.epoch_interruption(true);
        let engine = Engine::new(&config).expect("wasmtime engine config is static");
        let ticker = engine.clone();
        std::thread::Builder::new()
            .name("locowrk-wasm-epoch".into())
            .spawn(move || {
                loop {
                    std::thread::sleep(Duration::from_millis(abi::EPOCH_TICK_MS));
                    ticker.increment_epoch();
                }
            })
            .expect("spawn wasm epoch ticker");
        engine
    })
}

/// 只注入 `locowrk.log` 的 Linker：客户机声明任何其他导入（含 WASI）
/// 都会实例化失败——能力面由宿主枚举（§5.8 沙箱纪律）。
fn build_linker(engine: &Engine) -> Result<Linker<StoreLimits>, wasmtime::Error> {
    let mut linker: Linker<StoreLimits> = Linker::new(engine);
    linker.func_wrap(
        abi::HOST_IMPORT_MODULE,
        "log",
        // 闭包体不捕获任何状态（log_via_memory 是自由函数），保证满足
        // IntoFunc 的 `Fn` 约束；catch_unwind 的内层 FnOnce 逐次新建
        |mut caller: Caller<'_, StoreLimits>,
         level: i32,
         ptr: i32,
         len: i32|
         -> Result<(), wasmtime::Error> {
            // 宿主侧防线：catch_unwind 兜底（纪律 3 的导入一半）
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                log_via_memory(&mut caller, level, ptr, len);
            }));
            match outcome {
                Ok(()) => Ok(()),
                // host func 的 Result<_> 是 anyhow Error（WasmRet 约束）
                Err(_) => Err(Trap::UnreachableCodeReached.into()),
            }
        },
    )?;
    Ok(linker)
}

/// 从客户机线性内存读日志字节并按 level 路由到 tracing（镜像
/// host_vtable::host_log 的分级与占位文本语义）。
fn log_via_memory(caller: &mut Caller<'_, StoreLimits>, level: i32, ptr: i32, len: i32) {
    if ptr <= 0 || len <= 0 {
        return;
    }
    let Some(mem) = caller.get_export("memory").and_then(Extern::into_memory) else {
        return;
    };
    let len = (len as usize).min(abi::LOG_MAX_BYTES);
    let mut buf = vec![0u8; len];
    if mem.read(caller, ptr as usize, &mut buf).is_err() {
        return;
    }
    let msg = String::from_utf8_lossy(&buf);
    match level {
        l if l == log_level::ERROR as i32 => tracing::error!(target: "plugin", "{msg}"),
        l if l == log_level::WARN as i32 => tracing::warn!(target: "plugin", "{msg}"),
        l if l == log_level::DEBUG as i32 => tracing::debug!(target: "plugin", "{msg}"),
        _ => tracing::info!(target: "plugin", "{msg}"),
    }
}

/// 缺失导出的统一错误（附 ABI 清单便于排障）。
fn missing(path: &Path, name: &str) -> LoadError {
    LoadError::Wasm {
        path: path.display().to_string(),
        message: format!(
            "missing export `{name}` (ABI requires memory/describe/alloc/free/init/call/shutdown)"
        ),
    }
}

fn func<P, R>(
    instance: &wasmtime::Instance,
    mut store: impl wasmtime::AsContextMut,
    path: &Path,
    name: &str,
) -> Result<TypedFunc<P, R>, LoadError>
where
    P: wasmtime::WasmParams,
    R: wasmtime::WasmResults,
{
    instance
        .get_typed_func(&mut store, name)
        .map_err(|_| missing(path, name))
}

/// 按 NUL 结尾约定读 describe JSON：扫描上限 [abi::DESCRIBE_SCAN_MAX]，
/// 越界指针 / 无 NUL 都报错（恶意客户机骗不到宿主）。
fn read_nul_terminated(
    memory: &Memory,
    store: &Store<StoreLimits>,
    ptr: i32,
) -> Result<Vec<u8>, String> {
    let size = memory.data_size(store);
    if ptr < 0 || ptr as usize >= size {
        return Err(format!(
            "pointer {ptr} outside linear memory ({size} bytes)"
        ));
    }
    let cap = abi::DESCRIBE_SCAN_MAX.min(size - ptr as usize);
    let mut buf = vec![0u8; cap];
    memory
        .read(store, ptr as usize, &mut buf)
        .map_err(|e| format!("read: {e}"))?;
    match buf.iter().position(|&b| b == 0) {
        Some(i) => Ok(buf[..i].to_vec()),
        None => Err(format!(
            "JSON has no NUL terminator within {cap} bytes (abi.rs ownership rules)"
        )),
    }
}
