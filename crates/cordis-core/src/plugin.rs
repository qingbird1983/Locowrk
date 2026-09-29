//! Plugin：插件生命周期接口（DESIGN.md §3.3、附录 A）。
//!
//! 三段生命周期：`inject` 声明依赖（未满足则 runtime 拒绝激活）→
//! `apply` 激活（runtime 在当前 Fiber 作用域内调用，此处登记的服务
//! 自动按 FiberId 打 Effect 标签，撤销链可精确回收）→ `dispose` 撤销
//! （撤销链执行完毕后调用）。

use std::any::TypeId;

use crate::context::Context;

/// 插件生命周期接口：inject / apply / dispose（附录 A）。
#[async_trait::async_trait]
pub trait Plugin: Send + Sync + 'static {
    /// 声明依赖的服务类型；依赖未满足时 runtime 拒绝激活（§3.3 状态图）。
    fn inject(&self) -> Vec<TypeId> {
        Vec::new()
    }

    /// 插件激活：所有依赖满足后调用。服务注册必须在此完成，
    /// 才能被撤销链精确回收（§3.2）。
    async fn apply(&self, ctx: &Context) -> Result<(), PluginError>;

    /// 插件卸载：撤销链执行完毕后调用。
    async fn dispose(&self, ctx: &Context) -> Result<(), PluginError>;
}

/// 插件生命周期错误。runtime 捕获后把 Fiber 落 `FiberState::Failed`，
/// 详情写入 `Fiber::failure`（独立字段，状态枚举保持 Copy）。
#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("dependency {0:?} is missing")]
    DependencyMissing(TypeId),
    #[error("apply failed: {0}")]
    Apply(String),
    #[error("dispose failed: {0}")]
    Dispose(String),
}
