//! FFI → Tool 适配（DESIGN.md §5.5 `SabiToolAdapter`）+ 工具目录视角。
//!
//! 依赖边界说明（§7.2 / §8）：kernel-agent 定义 `ToolSource` trait，
//! 但依赖图禁止 plugin-host 依赖 kernel-agent（两者互不依赖）。因此
//! plugin-host 提供同语义的固有方法 [`RegistryToolSource::resolve`]，
//! 由组装点（kernel-bin / desktop）把它桥接成 `kernel_agent::ToolSource`
//! 服务注册进 Context——"主循环对 dlopen / Registry 一无所知"不变。

use std::sync::Arc;

use async_trait::async_trait;
use plugin_api::{CallCtx, Tool, ToolError, ToolSpec};
use serde_json::Value;
use tokio::sync::Semaphore;

use crate::instance::PluginInstance;
use crate::registry::PluginRegistry;

/// 把一个已加载插件实例的某个工具适配成宿主侧 `Tool`（§5.5）。
pub struct SabiToolAdapter {
    inner: Arc<PluginInstance>,
    spec: ToolSpec,
    sem: Arc<Semaphore>,
}

impl SabiToolAdapter {
    pub fn new(inner: Arc<PluginInstance>, spec: ToolSpec) -> Self {
        Self {
            sem: inner.semaphore(),
            inner,
            spec,
        }
    }
}

#[async_trait]
impl Tool for SabiToolAdapter {
    fn spec(&self) -> &ToolSpec {
        &self.spec
    }

    fn plugin_version(&self) -> &semver::Version {
        self.inner.version()
    }

    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        // 并发限制：permit 与实例绑定（同实例的所有适配器共享限额）
        let _permit = self
            .sem
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ToolError::Failed {
                tool: self.spec.name.clone(),
                message: "plugin semaphore closed".into(),
            })?;
        let inst = Arc::clone(&self.inner);
        let name = self.spec.name.clone();
        let name_for_task = name.clone();
        // 同步 FFI 丢到 blocking 池，不阻塞 reactor（§5.6 纪律三）
        tokio::task::spawn_blocking(move || inst.call_blocking(&name_for_task, &args))
            .await
            .map_err(|e| ToolError::Failed {
                tool: name,
                message: format!("blocking pool join failed: {e}"),
            })?
    }
}

/// 注册表的工具目录视角：按工具名在 active 实例中解析适配器。
/// 同名工具多插件声明时按插件 id 排序取第一个（确定性）。
pub struct RegistryToolSource {
    registry: Arc<PluginRegistry>,
}

impl RegistryToolSource {
    pub fn new(registry: Arc<PluginRegistry>) -> Self {
        Self { registry }
    }

    pub fn resolve(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError> {
        for (_id, inst) in self.registry.active_instances() {
            if let Some(spec) = inst.spec_of(name) {
                return Ok(Arc::new(SabiToolAdapter::new(inst, spec)));
            }
        }
        Err(ToolError::NotAvailable(name.to_string()))
    }
}
