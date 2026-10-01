//! 白名单工具视图（§7.2 作用域隔离的 ⑮ 实例）：把父 Agent 的目录收缩成
//! 显式子集给子 Agent。demo4 双 Agent 视图隔离的同类机制，这里是
//! "运行期派生作用域"——子 Agent 的 `ToolSource` 只看见白名单内的工具。

use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use plugin_api::{Tool, ToolError, ToolSpec};

use crate::kernel::ToolSource;

/// 子 Agent 自身（单层派生的硬底线：白名单显式包含也剔除——
/// 子不可再生子，防失控递归）。
pub const SELF_TOOL: &str = "subagent_run";

/// 父目录的白名单过滤包装。`allow` 为空 = 继承父全目录（仍剔除自身）；
/// 非空 = 严格子集（与父目录的交集，白名单里父没有的名字解析时报
/// NotAvailable，不泄漏父目录内容）。
pub struct FilterToolSource {
    inner: Arc<dyn ToolSource>,
    allow: Arc<HashSet<String>>,
}

impl FilterToolSource {
    pub fn new(inner: Arc<dyn ToolSource>, allow: Vec<String>) -> Self {
        Self {
            inner,
            allow: Arc::new(allow.into_iter().collect()),
        }
    }

    fn permitted(&self, name: &str) -> bool {
        name != SELF_TOOL && (self.allow.is_empty() || self.allow.contains(name))
    }
}

#[async_trait]
impl ToolSource for FilterToolSource {
    async fn resolve(&self, name: &str) -> Result<Arc<dyn Tool>, ToolError> {
        if !self.permitted(name) {
            return Err(ToolError::NotAvailable(name.to_string()));
        }
        self.inner.resolve(name).await
    }

    async fn list(&self) -> Vec<ToolSpec> {
        let mut specs = Vec::new();
        for spec in self.inner.list().await {
            if self.permitted(&spec.name) {
                specs.push(spec);
            }
        }
        specs
    }
}

#[cfg(test)]
mod tests;
