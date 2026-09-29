//! 加载失败与版本管理错误（DESIGN.md §5.7 LoadError）。

use plugin_api::ManifestError;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("open dylib `{path}`: {source}")]
    Open {
        path: String,
        #[source]
        source: libloading::Error,
    },
    #[error("resolve symbol `{symbol}`: {source}")]
    Symbol {
        symbol: String,
        #[source]
        source: libloading::Error,
    },
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error("plugin describe panicked (caught by host)")]
    DescribePanicked,
    #[error("manifest version `{manifest}` is not valid semver")]
    BadVersion { manifest: String },
    #[error(
        "abi mismatch: host expects {expected}, plugin built for {found} \
         (rebuild the plugin against this plugin-api)"
    )]
    AbiMismatch { expected: u32, found: u32 },
    #[error("manifest id `{manifest}` does not match requested `{expected}`")]
    IdMismatch { expected: String, manifest: String },
    #[error("manifest version `{manifest}` does not match requested `{expected}`")]
    VersionMismatch { expected: String, manifest: String },
    #[error("plugin init failed (describe ok, init returned null or panicked)")]
    InitFailed,
    #[error("wasm plugin `{path}`: {message}")]
    Wasm { path: String, message: String },
    #[error("plugin `{0}` is not loaded")]
    NotLoaded(String),
    #[error("plugin `{0}` has no previous version to roll back to")]
    NoPreviousVersion(String),
}
