//! 桥接层：把 plugin-host 的工具目录适配成 kernel-agent 的 `ToolSource`
//! 服务（§7.2）。依赖图禁止 plugin-host 实现 kernel-agent 的 trait
//! （两者互不依赖），所以这段胶水只能住在组装点（kernel-bin / desktop）。
//!
//! 组装点内置工具（如阶段六演示用 send_email）优先于插件注册表：
//! 同名冲突时内置版本赢，插件不能用演示替身的名字顶掉它。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_agent::ToolSource;
use plugin_api::{Tool, ToolError, ToolSpec};
use plugin_host::RegistryToolSource;

/// 注册表 → 工具目录服务。`resolve` 先查内置表，再委托注册表实现。
pub struct KernelToolSource {
    builtins: Vec<(String, Arc<dyn Tool>)>,
    inner: RegistryToolSource,
}

impl KernelToolSource {
    pub fn new(inner: RegistryToolSource) -> Self {
        Self {
            builtins: Vec::new(),
            inner,
        }
    }

    /// 追加一个组装点内置工具（演示替身；真实实现应来自 plugins/）。
    pub fn with_builtin(mut self, tool: Arc<dyn Tool>) -> Self {
        self.builtins.push((tool.spec().name.clone(), tool));
        self
    }
}

#[async_trait]
impl ToolSource for KernelToolSource {
    async fn resolve(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError> {
        if let Some((_, tool)) = self.builtins.iter().find(|(n, _)| n == name) {
            return Ok(Arc::clone(tool));
        }
        self.inner.resolve(name)
    }

    async fn list(&self) -> Vec<ToolSpec> {
        // 与 resolve 同优先级：内置替身先出现，插件不得顶掉同名内置
        let mut specs: Vec<ToolSpec> = self
            .builtins
            .iter()
            .map(|(_, t)| t.spec().clone())
            .collect();
        for spec in self.inner.list_tools() {
            if !specs.iter().any(|s| s.name == spec.name) {
                specs.push(spec);
            }
        }
        specs
    }
}
