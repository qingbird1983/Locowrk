//! 桥接层：把 plugin-host 的工具目录适配成 kernel-agent 的 `ToolSource`
//! 服务（§7.2）。依赖图禁止 plugin-host 实现 kernel-agent 的 trait
//! （两者互不依赖），所以这段胶水只能住在组装点（kernel-bin / desktop）。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_agent::ToolSource;
use plugin_api::{Tool, ToolError};
use plugin_host::RegistryToolSource;

/// 注册表 → 工具目录服务。`resolve` 直接委托固有方法实现。
pub struct KernelToolSource {
    inner: RegistryToolSource,
}

impl KernelToolSource {
    pub fn new(inner: RegistryToolSource) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl ToolSource for KernelToolSource {
    async fn resolve(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError> {
        self.inner.resolve(name)
    }
}
