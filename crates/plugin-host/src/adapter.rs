//! FFI → Tool 适配（DESIGN.md §5.5 `SabiToolAdapter`）+ 工具目录视角。
//!
//! 依赖边界说明（§7.2 / §8）：kernel-agent 定义 `ToolSource` trait，
//! 但依赖图禁止 plugin-host 依赖 kernel-agent（两者互不依赖）。因此
//! plugin-host 提供同语义的固有方法 [`RegistryToolSource::resolve`]，
//! 由组装点（kernel-bin / desktop）把它桥接成 `kernel_agent::ToolSource`
//! 服务注册进 Context——"主循环对 dlopen / Registry 一无所知"不变。

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use plugin_api::{CallCtx, Exposure, Tool, ToolError, ToolSpec};
use serde_json::Value;
use tokio::sync::Semaphore;

use crate::instance::PluginInstance;
use crate::registry::PluginRegistry;

/// always>10 预算告警只发一次（exposure 文档 §4.1：作者设计失误）。
static ALWAYS_BUDGET_WARNED: OnceLock<()> = OnceLock::new();

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

    /// manifest.id 即插件 id（registry 加载时已校验与 lockfile 键一致）。
    fn plugin_id(&self) -> Option<&str> {
        Some(self.inner.id())
    }

    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
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
        let session_id = ctx.session_id.clone();
        // 同步 FFI 丢到 blocking 池，不阻塞 reactor（§5.6 纪律三）。
        // ㉘：blocking 线程上预置调用期会话上下文（session_id + 解析出的
        // 项目根），插件经 vtable `session_context` 反向查询；RAII guard
        // 保证 panic / 提前返回也清干净。解析器缺席（headless）= 全缺省。
        // ㉘·B：解析器一并返回 fs_roots（围栏跟会话）。
        tokio::task::spawn_blocking(move || {
            let ctx = session_id
                .as_deref()
                .map(crate::host_vtable::session_context_of)
                .unwrap_or_default();
            let _guard = crate::host_vtable::call_context_guard(plugin_api::ffi::CallContext {
                session_id: ctx.session_id,
                workspace: ctx.workspace,
                fs_roots: ctx.fs_roots,
            });
            inst.call_blocking(&name_for_task, &args)
        })
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

    /// 全目录快照（供 LLM 请求枚举工具）：与 `resolve` 同序——active 实例
    /// 按 id 排序，同名工具先声明者赢。暴露管线直通态（⑩②）：hidden 不进
    /// 常规目录（只等 ⑫ 检索激活）；always 直通 + on_demand 全暴露，行为
    /// 与加字段前一致（截断归 ⑫）。always 总数超 10 一次性告警（§4.1）。
    pub fn list_tools(&self) -> Vec<ToolSpec> {
        self.list_impl(false)
    }

    /// 全目录含 hidden（exposure 文档 §4.1"hidden 只有显式检索才暴露"）：
    /// search_tools 元工具的检索索引（⑫）专用——hidden 不进常规目录，但
    /// LLM 主动检索时要找得到。resolve 本就不过滤，找到即可调。
    pub fn list_all_tools(&self) -> Vec<ToolSpec> {
        self.list_impl(true)
    }

    fn list_impl(&self, include_hidden: bool) -> Vec<ToolSpec> {
        let mut specs = Vec::new();
        let mut always = 0usize;
        for (_id, inst) in self.registry.active_instances() {
            let (merged, a, invalid) =
                merge_catalog(specs, inst.manifest().tools.iter().cloned(), include_hidden);
            specs = merged;
            always += a;
            if !invalid.is_empty() {
                // 非法名不剔除就会让整个 chat 请求 400；剔除只损失该工具
                tracing::warn!(
                    plugin = %inst.id(),
                    names = %invalid.join(", "),
                    "skipped tool(s) with names outside ^[a-zA-Z0-9_-]+$ \
                     (OpenAI-compatible backends reject the whole request)"
                );
            }
        }
        if !include_hidden && always > 10 && ALWAYS_BUDGET_WARNED.set(()).is_ok() {
            tracing::warn!(
                count = always,
                "always-exposure tools exceed the 10 budget (exposure doc §4.1) — \
                 authors should declare on_demand/hidden instead"
            );
        }
        specs
    }
}

/// 目录合并纯函数（单测友好）：非法名剔除（名单回报给调用方告警）、
/// hidden 按 `include_hidden` 决定去留、同名先到先得，返回（合并后的
/// 目录，本批 always 计数，非法名清单）。`resolve` 不过滤——补偿等内部
/// 按名引用仍可达。
fn merge_catalog(
    existing: Vec<ToolSpec>,
    incoming: impl Iterator<Item = ToolSpec>,
    include_hidden: bool,
) -> (Vec<ToolSpec>, usize, Vec<String>) {
    let mut specs = existing;
    let mut always = 0usize;
    let mut invalid = Vec::new();
    for spec in incoming {
        if !plugin_api::is_valid_tool_name(&spec.name) {
            invalid.push(spec.name);
            continue;
        }
        if !include_hidden && matches!(spec.exposure, Exposure::Hidden) {
            continue;
        }
        // always 预算数"实际入目录"的：被同名先声明者顶掉的不计（§4.1）
        let duplicated = specs.iter().any(|s| s.name == spec.name);
        if !duplicated {
            if matches!(spec.exposure, Exposure::Always) {
                always += 1;
            }
            specs.push(spec);
        }
    }
    (specs, always, invalid)
}

#[cfg(test)]
mod tests;
