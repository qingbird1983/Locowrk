//! 桥接层：把 plugin-host 的工具目录适配成 kernel-agent 的 `ToolSource`
//! 服务（§7.2）。依赖图禁止 plugin-host 实现 kernel-agent 的 trait
//! （两者互不依赖），所以这段胶水只能住在组装点（kernel-bin / desktop）。
//!
//! 组装点内置工具（如阶段六演示用 send_email）优先于插件注册表：
//! 同名冲突时内置版本赢，插件不能用演示替身的名字顶掉它。
//! ㉘·B：`resolve` / `list` 带会话身份——**计划模式 = 目录级围栏**（非
//! Pure 工具对模型不可见、解析即拒），由 [`PermissionModeStore`] 按会话
//! 裁决；未接模式存储时不过滤（与 kernel-bin 桥同形）。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_agent::ToolSource;
use plugin_api::{SideEffect, Tool, ToolError, ToolSpec};
use plugin_host::RegistryToolSource;

use crate::organizer_mode::{OrganizerMode, OrganizerModeStore};
use crate::permission_mode::{PermissionMode, PermissionModeStore};

/// M4 树工具（memory_expand / memory_collapse）的会话门控数据源。
pub struct TreeToolGate {
    modes: Arc<OrganizerModeStore>,
    /// memory_collapse 暴露开关（settings `memory.collapseEnabled`，保存即
    /// 读当前值；缺省关——§8.3 宁缺勿假）。
    collapse_enabled: Arc<dyn Fn() -> bool + Send + Sync>,
}

impl TreeToolGate {
    pub fn new(
        modes: Arc<OrganizerModeStore>,
        collapse_enabled: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        Self {
            modes,
            collapse_enabled,
        }
    }

    /// 工具对该会话是否可见/可解析：expand 树会话常驻；collapse 树会话 +
    /// 开关开启。
    fn allows(&self, session_id: Option<&str>, name: &str) -> bool {
        let tree = self.modes.mode_of(session_id) == OrganizerMode::Tree;
        match name {
            "memory_expand" => tree,
            "memory_collapse" => tree && (self.collapse_enabled)(),
            _ => true,
        }
    }
}

/// 注册表 → 工具目录服务。`resolve` 先查内置表，再委托注册表实现。
pub struct KernelToolSource {
    builtins: Vec<(String, Arc<dyn Tool>)>,
    inner: RegistryToolSource,
    /// ㉘·B 会话级权限模式（计划模式目录围栏）；None = 不过滤。
    modes: Option<Arc<PermissionModeStore>>,
    /// M4 树工具会话门控；None = 无树工具（kernel-bin 桥同形）。
    tree_gate: Option<Arc<TreeToolGate>>,
}

impl KernelToolSource {
    pub fn new(inner: RegistryToolSource) -> Self {
        Self {
            builtins: Vec::new(),
            inner,
            modes: None,
            tree_gate: None,
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

    /// M4：接树工具门控（memory_expand / memory_collapse 按会话整理模式
    /// 过滤；legacy 会话目录不含、解析即拒——双保险）。
    pub fn with_tree_gate(mut self, gate: Arc<TreeToolGate>) -> Self {
        self.tree_gate = Some(gate);
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
        if let Some(gate) = &self.tree_gate {
            if !gate.allows(session_id, name) {
                return Err(ToolError::NotAvailable(format!(
                    "{name} 不可用：仅记忆树整理模式的会话可调（任务行菜单可切换）"
                )));
            }
        }
        Ok(tool)
    }

    async fn list(&self, session_id: Option<&str>) -> Vec<ToolSpec> {
        // 与 resolve 同优先级：内置替身先出现，插件不得顶掉同名内置
        let mut specs: Vec<ToolSpec> = self
            .builtins
            .iter()
            .map(|(_, t)| t.spec().clone())
            .filter(|s| {
                self.tree_gate
                    .as_ref()
                    .is_none_or(|g| g.allows(session_id, &s.name))
            })
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
