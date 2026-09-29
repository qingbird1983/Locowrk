//! 插件加载器抽象（DESIGN.md §5.8）：`PluginLoader` trait + 第一实现
//! `DylibLoader`。
//!
//! 加载序列：open → describe → manifest 解析 → ABI 校验 → 符号解析
//! → init。init 之前的任何失败都走"加载即失败"豁免：实例从未交出
//! 代码指针，Library 随错误 Drop（dlclose）是安全的（§5.6 纪律一）。
//! WASM 侧的第二实现在 `wasm` 模块；registry 按 `.wasm` 扩展名分发。

use std::ffi::{CStr, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::sync::Arc;

use libloading::{Library, Symbol};
use plugin_api::manifest::PluginManifest;
use plugin_api::{PLUGIN_ABI_VERSION, ffi};

use crate::error::LoadError;
use crate::host_vtable::host_vtable;
use crate::instance::PluginInstance;

/// 插件加载器抽象（§5.8）：内核只依赖此 trait，不感知具体加载方式。
/// `path` 指向加载工件（dll/so/dylib 或 .wasm）。
pub trait PluginLoader: Send + Sync {
    fn load(&self, path: &Path) -> Result<Arc<PluginInstance>, LoadError>;
}

/// 动态库加载器（§5.8 的 `DylibLoader` 实现）。
pub struct DylibLoader;

impl DylibLoader {
    /// dlopen 一个插件并完成完整握手。成功即返回可调用实例。
    pub fn load(path: &Path) -> Result<PluginInstance, LoadError> {
        // SAFETY: 动态库加载；插件代码的正确性由 ABI 校验 + catch_unwind 兜底
        let lib = unsafe { Library::new(path) }.map_err(|source| LoadError::Open {
            path: path.display().to_string(),
            source,
        })?;

        let manifest_bytes = unsafe { describe_bytes(&lib) }?;
        let manifest = PluginManifest::from_json(manifest_bytes)?;
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

        // SAFETY: 符号由 ABI 约定签名；签名不符属插件缺陷，由 catch_unwind 兜底
        let (call, shutdown, free) = unsafe {
            (
                *resolve::<ffi::CallFn>(&lib, ffi::SYM_CALL)?,
                *resolve::<ffi::ShutdownFn>(&lib, ffi::SYM_SHUTDOWN)?,
                *resolve::<ffi::FreeBufFn>(&lib, ffi::SYM_FREE)?,
            )
        };

        let init: ffi::InitFn = *unsafe { resolve::<ffi::InitFn>(&lib, ffi::SYM_INIT)? };
        let handle = catch_unwind(AssertUnwindSafe(|| init(host_vtable())))
            .map_err(|_| LoadError::InitFailed)?;
        if handle.is_null() {
            return Err(LoadError::InitFailed);
        }

        Ok(PluginInstance::new(
            manifest,
            version,
            Box::new(crate::instance::NativeBackend::new(
                call, shutdown, free, handle, lib,
            )),
            path.to_path_buf(),
        ))
    }
}

impl PluginLoader for DylibLoader {
    fn load(&self, path: &Path) -> Result<Arc<PluginInstance>, LoadError> {
        Ok(Arc::new(Self::load(path)?))
    }
}

/// `.wasm` 工件判别（registry 分发的唯一依据；大小写不敏感）。
pub(crate) fn is_wasm_artifact(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("wasm"))
}

/// 调用 describe 并按 NUL 结尾约定截取 JSON 字节（§5.1 长度协议）。
///
/// # Safety
/// `lib` 必须已成功打开且导出符合 ABI 的 describe 符号。
unsafe fn describe_bytes(lib: &Library) -> Result<&'static [u8], LoadError> {
    let describe: ffi::DescribeFn = *unsafe { resolve::<ffi::DescribeFn>(lib, ffi::SYM_DESCRIBE)? };
    // SAFETY: describe 是宿主刚解析出的合法函数指针；返回值按约定
    // 指向插件内的静态 NUL 结尾字节串（生命周期 = 库存活期 = 实例存活期）
    let ptr =
        catch_unwind(AssertUnwindSafe(|| describe())).map_err(|_| LoadError::DescribePanicked)?;
    // SAFETY: 契约保证返回 NUL 结尾的 UTF-8 静态字节串
    let cstr = unsafe { CStr::from_ptr(ptr as *const c_char) };
    Ok(cstr.to_bytes())
}

/// # Safety
/// `lib` 已打开；`sym` 是 NUL 结尾符号名。
unsafe fn resolve<'lib, T: Copy + 'lib>(
    lib: &'lib Library,
    sym: &[u8],
) -> Result<Symbol<'lib, T>, LoadError> {
    // SAFETY: 由调用方契约保证
    unsafe { lib.get::<T>(sym) }.map_err(|source| LoadError::Symbol {
        symbol: String::from_utf8_lossy(&sym[..sym.len() - 1]).into_owned(),
        source,
    })
}
