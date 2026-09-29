//! 统一错误类型（cordis-core 内部；附录 A 的 ServiceMissing / EffectNotFound）。

use std::any::TypeId;

use crate::fiber::FiberState;
use crate::plugin::PluginError;

/// cordis-core 统一错误。
#[derive(Debug, thiserror::Error)]
pub enum CordisError {
    #[error("service `{service}` is not provided")]
    ServiceMissing { service: &'static str },
    #[error("effect {id} not found (already revoked?)")]
    EffectNotFound { id: u64 },
    /// 激活门拒绝：inject 声明的服务键尚无提供者（§3.3）。
    #[error("dependency {0:?} is not satisfied")]
    DependencyMissing(TypeId),
    /// 插件 apply/dispose 失败（runtime 已把 Fiber 落 Failed）。
    #[error("plugin error: {0}")]
    Plugin(#[from] PluginError),
    /// 状态机非法迁移（§15.4：报错而非静默降级）。
    #[error("fiber {fiber}: illegal transition {from:?} → {to:?}")]
    IllegalTransition {
        fiber: u64,
        from: FiberState,
        to: FiberState,
    },
}

impl CordisError {
    /// `use_service` / `use_dyn` 的缺服务错误（§3.1 的 `ServiceMissing::new::<S>`）。
    pub fn service_missing<T: ?Sized>() -> Self {
        Self::ServiceMissing {
            service: std::any::type_name::<T>(),
        }
    }
}

pub type Result<T> = std::result::Result<T, CordisError>;
