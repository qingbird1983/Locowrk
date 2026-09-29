//! 插件运行实例：加载产物的统一门面（§5.5）。
//!
//! 后端二态（§5.8）：[`native`]（dlopen + C ABI）与 `wasm`（wasmtime
//! 沙箱，见 `wasm` 模块）。门面只暴露 manifest / version / path /
//! `call_blocking` / semaphore；资源释放由各后端自己的 Drop 负责——
//! native 的 Drop 是全进程唯一 dlclose 路径（§5.6 纪律一的"加载即失败"
//! 豁免）；wasm 的 Drop 做 best-effort guest shutdown（trap 静默吞掉，
//! 沙箱内本就无宿主资源可泄）。
//!
//! 并发契约差异：native 允许同实例多线程并发 call（插件自保证线程安全）；
//! wasm 客户机是单线程线性内存，同实例调用经 Mutex 串行（语义收窄，
//! 宿主侧接口不变）。

mod native;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use plugin_api::manifest::PluginManifest;
use plugin_api::{ToolError, ToolSpec};
use serde_json::Value;
use tokio::sync::Semaphore;

pub(crate) use native::NativeBackend;

/// 加载后端的内部契约：一次阻塞式工具调用（调用方负责 `spawn_blocking`，
/// §5.6 纪律三）。panic 防线在各后端内部（native catch_unwind；wasm
/// trap 天然不逃逸）。Send + Sync：实例以 `Arc` 共享给多线程适配器。
pub(crate) trait PluginBackend: Send + Sync {
    fn call_blocking(&self, tool: &str, args: &Value) -> Result<Value, ToolError>;
}

/// 一个已加载插件版本。共享形态固定为 `Arc<PluginInstance>`。
pub struct PluginInstance {
    manifest: PluginManifest,
    version: semver::Version,
    backend: Box<dyn PluginBackend>,
    /// 同实例的并发调用上限（§5.5：适配器持有 permit，实例共享限额）。
    sem: Arc<Semaphore>,
    /// 加载来源（dll/wasm 所在路径）。其父目录是 manifest.json 与图标的
    /// 定位基础（packaging.md §3.1：每个版本目录自包含）。
    path: PathBuf,
}

impl PluginInstance {
    /// loader 专用构造：所有字段已在加载路径上校验完毕。
    pub(crate) fn new(
        manifest: PluginManifest,
        version: semver::Version,
        backend: Box<dyn PluginBackend>,
        path: PathBuf,
    ) -> Self {
        Self {
            manifest,
            version,
            backend,
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

    /// 加载来源路径（工件所在位置；父目录即版本目录）。
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

    /// 同步阻塞调用（调用方必须放在 `spawn_blocking` 内，§5.6 纪律三）。
    pub(crate) fn call_blocking(&self, tool: &str, args: &Value) -> Result<Value, ToolError> {
        self.backend.call_blocking(tool, args)
    }
}

impl std::fmt::Debug for PluginInstance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // backend 是 trait object（无 Debug）；结构与路径足够诊断用
        f.debug_struct("PluginInstance")
            .field("manifest", &self.manifest)
            .field("version", &self.version)
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}
