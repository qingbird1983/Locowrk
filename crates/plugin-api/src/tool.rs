//! 宿主侧工具契约（DESIGN.md §5.2）。只活在宿主进程内，绝不进入 C ABI。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::de::{self, Deserializer, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 一次工具调用的标识。事件日志（session-store）与调用上下文共用，
/// 也是补偿的幂等去重键（§6.3 / 纪律 10）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CallId(pub String);

/// 副作用等级（§5.2）：内核据此决定补偿与审批策略。
///
/// 序列化固定 tagged 形态 `{"kind":"…"}`（describe JSON 是 ABI 边界，
/// 不破坏旧插件）；解析兼容双形态（packaging.md §2.1 与 §8.1 两处示例
/// 自身不一致，DESIGN §14 v2.10 对齐决定）：tagged 对象，或大小写不敏感
/// 的纯字符串 `"Pure"` / `"Irreversible"`。纯字符串无法携带 `undo_tool`，
/// 因此 `"Compensable"` 只接受 tagged 形态，纯字符串形式报错。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SideEffect {
    /// 只读，回退免费（§6.3）。
    Pure,
    /// 有补偿工具，按 `undo_tool` 逆序补偿（§6.3）。
    Compensable { undo_tool: String },
    /// 不可逆，调用前必须审批（纪律 9）。
    Irreversible,
}

impl<'de> Deserialize<'de> for SideEffect {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct SideEffectVisitor;

        impl<'de> Visitor<'de> for SideEffectVisitor {
            type Value = SideEffect;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(
                    "a side effect: \"Pure\"/\"Irreversible\" string or {\"kind\":...} object",
                )
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<SideEffect, E> {
                match v.trim().to_ascii_lowercase().as_str() {
                    "pure" => Ok(SideEffect::Pure),
                    "irreversible" => Ok(SideEffect::Irreversible),
                    "compensable" => Err(E::custom(
                        "plain-string `Compensable` cannot carry undo_tool; \
                         use {\"kind\":\"compensable\",\"undo_tool\":\"…\"}",
                    )),
                    other => Err(E::custom(format!("unknown side effect `{other}`"))),
                }
            }

            fn visit_map<M: MapAccess<'de>>(self, mut map: M) -> Result<SideEffect, M::Error> {
                let mut kind: Option<String> = None;
                let mut undo_tool: Option<String> = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "kind" => kind = Some(map.next_value()?),
                        "undo_tool" => undo_tool = Some(map.next_value()?),
                        // 未知字段忽略（前向兼容，§9 未来扩展）
                        _ => drop(map.next_value::<de::IgnoredAny>()?),
                    }
                }
                match kind.as_deref().map(str::to_ascii_lowercase).as_deref() {
                    Some("pure") => Ok(SideEffect::Pure),
                    Some("irreversible") => Ok(SideEffect::Irreversible),
                    Some("compensable") => match undo_tool {
                        Some(undo_tool) => Ok(SideEffect::Compensable { undo_tool }),
                        None => Err(de::Error::missing_field("undo_tool")),
                    },
                    other => Err(de::Error::custom(format!(
                        "unknown side effect kind {:?}",
                        other.unwrap_or("(missing)")
                    ))),
                }
            }
        }

        deserializer.deserialize_any(SideEffectVisitor)
    }
}

/// 工具规格：每个工具自带 side_effect，不再有 manifest 级冗余字段（§5.2）。
/// （PartialEq 仅供内核 payload 比较；derive 是 trait impl，不影响 C ABI。）
///
/// 序列化恒为单语言 `description: String`——这是模型侧的目录描述，不做
/// 多语言（DESIGN §14 v2.10 对齐决定）。解析宽容（packaging.md "任何字段
/// 可缺失"）：`description` 兼容多语言 map（就地按 en-US → 任意压平），
/// 除 `name` 外的字段缺失取缺省值，工具级 `display_name` 等未知字段忽略。
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON Schema。
    pub schema: Value,
    pub side_effect: SideEffect,
}

impl<'de> Deserialize<'de> for ToolSpec {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            name: String,
            #[serde(default)]
            description: Option<crate::manifest::LocalizedOrString>,
            #[serde(default)]
            schema: Value,
            #[serde(default = "default_side_effect")]
            side_effect: SideEffect,
        }
        fn default_side_effect() -> SideEffect {
            // 缺失按最保守处理：不可逆 → 调用前必须审批（宁可标错成
            // Irreversible，也不要标错成 Pure；AGENTS.md 设计纪律）
            SideEffect::Irreversible
        }
        let raw = Raw::deserialize(deserializer)?;
        Ok(ToolSpec {
            name: raw.name,
            description: raw
                .description
                .and_then(|d| d.pick("en-US").map(str::to_owned))
                .unwrap_or_default(),
            schema: raw.schema,
            side_effect: raw.side_effect,
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("tool `{0}` is not available")]
    NotAvailable(String),
    #[error("tool `{tool}` failed: {message}")]
    Failed { tool: String, message: String },
}

/// 一次工具调用的宿主侧上下文（§5.2：session_id / call_id / deadline / 取消信号）。
#[derive(Clone)]
pub struct CallCtx {
    pub session_id: Option<String>,
    pub call_id: Option<CallId>,
    pub deadline: Option<Instant>,
    cancelled: Arc<AtomicBool>,
}

impl CallCtx {
    pub fn new() -> Self {
        Self {
            session_id: None,
            call_id: None,
            deadline: None,
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Default for CallCtx {
    fn default() -> Self {
        Self::new()
    }
}

/// 宿主侧工具 trait（§5.2）。plugin-host 的 FFI 适配器（阶段三）与内核测试的
/// mock 工具都实现它；`plugin_version` 供事件日志与版本回退对表。
#[async_trait::async_trait]
pub trait Tool: Send + Sync + 'static {
    fn spec(&self) -> &ToolSpec;
    fn plugin_version(&self) -> &semver::Version;
    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError>;
}

#[cfg(test)]
mod tests;
