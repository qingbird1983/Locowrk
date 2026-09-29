//! 统一错误类型（cordis-core 内部；附录 A 的 ServiceMissing / EffectNotFound）。

/// cordis-core 统一错误。
#[derive(Debug, thiserror::Error)]
pub enum CordisError {
    #[error("service `{service}` is not provided")]
    ServiceMissing { service: &'static str },
    #[error("effect {id} not found (already revoked?)")]
    EffectNotFound { id: u64 },
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
