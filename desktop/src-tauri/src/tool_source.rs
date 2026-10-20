//! 桥接层：把 plugin-host 的工具目录适配成 kernel-agent 的 `ToolSource`
//! 服务（§7.2）。依赖图禁止 plugin-host 实现 kernel-agent 的 trait
//! （两者互不依赖），所以这段胶水只能住在组装点（kernel-bin / desktop）。
//!
//! 组装点内置工具（如阶段六演示用 send_email）优先于插件注册表：
//! 同名冲突时内置版本赢，插件不能用演示替身的名字顶掉它。
//! ㉘·B：`resolve` / `list` 带会话身份——**计划模式 = 目录级围栏**（非
//! Pure 工具对模型不可见、解析即拒），由 [`PermissionModeStore`] 按会话
//! 裁决；未接模式存储时不过滤（与 kernel-bin 桥同形）。
//!
//! Q1b（召回可观测性·检索回执）：`resolve` 是**执行面唯一的收敛出口**（内置工具与
//! 插件工具在此之后是同一个 `Arc<dyn Tool>`），所以采集壳挂在这里而不是任何工具本体
//! 或 `run_turn` 的参数上——理由与排除的两条路写在 `recall_log/mod.rs` 头注。未接观测句柄
//! 时一行都不记（kernel-bin 桥与测试路径同形）。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_agent::ToolSource;
use plugin_api::{SideEffect, Tool, ToolError, ToolSpec};
use plugin_host::RegistryToolSource;

use crate::permission_mode::{PermissionMode, PermissionModeStore};

/// 注册表 → 工具目录服务。`resolve` 先查内置表，再委托注册表实现。
pub struct KernelToolSource {
    builtins: Vec<(String, Arc<dyn Tool>)>,
    inner: RegistryToolSource,
    /// ㉘·B 会话级权限模式（计划模式目录围栏）；None = 不过滤。
    modes: Option<Arc<PermissionModeStore>>,
    /// Q1b 检索回执的采集件（观测表 + 键位解析器，两者必须同批给）；
    /// None = 不采集（`resolve` 原样返回工具）。
    receipts: Option<Arc<crate::recall_receipt::ReceiptSink>>,
}

impl KernelToolSource {
    pub fn new(inner: RegistryToolSource) -> Self {
        Self {
            builtins: Vec::new(),
            inner,
            modes: None,
            receipts: None,
        }
    }

    /// 追加一个组装点内置工具（演示替身；真实实现应来自 plugins/）。
    pub fn with_builtin(mut self, tool: Arc<dyn Tool>) -> Self {
        self.builtins.push((tool.spec().name.clone(), tool));
        self
    }

    /// ㉘·B：接会话级权限模式存储（计划模式目录围栏的数据源）。
    pub fn with_modes(mut self, modes: Arc<PermissionModeStore>) -> Self {
        self.modes = Some(modes);
        self
    }

    /// Q1b：接检索回执的采集件（`memory_read` / `memory_search` /
    /// `memory_project_history` 的成功结果落一行，见 [`crate::recall_receipt`]）。
    /// 只接**执行面**那一棵树：`state/tools.rs` 里的 `search_catalog` 不接——它只被
    /// `search_tools` 拿去 `list_all()` 建索引，从不调 `resolve`，接了也不会有读数，
    /// 但会把"这棵树是干什么的"讲成一个错误的故事。
    pub fn with_receipt_sink(mut self, sink: Arc<crate::recall_receipt::ReceiptSink>) -> Self {
        self.receipts = Some(sink);
        self
    }

    /// 计划模式且该会话命中 = 只放行 Pure 工具（"模型看不见的工具才
    /// 不会去调"；调到了也被 resolve 拒掉，双保险）。
    fn plan_locked(&self, session_id: Option<&str>) -> bool {
        self.modes
            .as_ref()
            .is_some_and(|m| m.mode_of(session_id) == PermissionMode::Plan)
    }
}

#[async_trait]
impl ToolSource for KernelToolSource {
    async fn resolve(
        &self,
        session_id: Option<&str>,
        name: &str,
    ) -> Result<Arc<dyn Tool>, ToolError> {
        let tool = if let Some((_, tool)) = self.builtins.iter().find(|(n, _)| n == name) {
            Arc::clone(tool)
        } else {
            self.inner.resolve(name)?
        };
        if self.plan_locked(session_id) && !matches!(tool.spec().side_effect, SideEffect::Pure) {
            return Err(ToolError::NotAvailable(format!(
                "{name} 在计划模式下不可用：计划模式只读，写类工具已被目录收起"
            )));
        }
        // Q1b：围栏全部放行之后才套采集壳——被拒的调用没有结果可观测，记了就是把
        // 「模型没查到」冒充「查到了零条」。
        Ok(match &self.receipts {
            Some(sink) => crate::recall_receipt::observe(tool, Arc::clone(sink)),
            None => tool,
        })
    }

    async fn list(&self, session_id: Option<&str>) -> Vec<ToolSpec> {
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
        if self.plan_locked(session_id) {
            specs.retain(|s| matches!(s.side_effect, SideEffect::Pure));
        }
        specs
    }
}

impl KernelToolSource {
    /// 全目录含 hidden（⑫ search_tools 检索索引用，exposure 文档 §4.1
    /// "hidden 只有显式检索才暴露"）：内置工具 + 注册表全量。停用插件
    /// 不在 active 实例里，天然不在索引（§8.2）。不做会话过滤——检索
    /// 索引是人的查询面，执行侧由 resolve 的计划围栏兜底。
    pub fn list_all(&self) -> Vec<ToolSpec> {
        let mut specs: Vec<ToolSpec> = self
            .builtins
            .iter()
            .map(|(_, t)| t.spec().clone())
            .collect();
        for spec in self.inner.list_all_tools() {
            if !specs.iter().any(|s| s.name == spec.name) {
                specs.push(spec);
            }
        }
        specs
    }
}
