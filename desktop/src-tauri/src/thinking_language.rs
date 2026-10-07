//! 思考语言注入（docs/design/NEW/language of thought.md）：`agent/request`
//! waterfall 监听器，把「思考过程用什么语言」的提示词追加到 system 尾部。
//!
//! 设计对齐文档：
//! - **不是插件、不是内核中间件**：宿主内置监听器（TreeDirectoryInject
//!   同款注册方式），配置是设置项 `thinking.language`（§1 不跨 ABI）。
//! - **链位 = system 真尾部**（§6）：注册在 TreeDirectoryInject 之后、
//!   ctx-compact 之前——目录之后、会话历史之前，对所有会话同一位置。
//! - **会话内锁死**（§5 前缀缓存红线）：按 session_id 首请求解析一次并
//!   缓存；中途改设置当前会话不变，新会话用新配置。None 身份（headless /
//!   无身份请求）也缓存一条（M1 同款）。
//! - 「跟随界面」（§3）：settings.locale（zh-CN / en），`system` 档经
//!   sys-locale 解析 OS 语言（tray.rs 同源）。
//! - 提示词模板在 `prompts/thinking_language/*.md`，编译期 include_str!
//!   （memory-tree prompts 同款——单文件分发前提下的「不硬编码」）。
//!
//! 缓存驱逐边界（记档）：FIFO 128 槽，被驱逐的超长会话会重新解析、可能
//! 吃到新配置（一次计划内前缀失效）——与 Memory-Inject 预取同口径。

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use cordis_core::{WaterfallDecision, WaterfallListener};
use kernel_agent::{points, RequestPayload};
use serde_json::Value;

use crate::settings::{SettingsStore, THINKING_NONE};

const ZH_HINT: &str = include_str!("prompts/thinking_language/zh-CN.md");
const EN_HINT: &str = include_str!("prompts/thinking_language/en-US.md");
// none.md 是空文件：none 档也从模板读，保持「内容全部来自 prompts/」
const NONE_HINT: &str = include_str!("prompts/thinking_language/none.md");

/// 预解析缓存上限（FIFO 驱逐最旧）：与 Memory-Inject 预取 / ToolExposure
/// 钉死表同款保险丝。
const CACHE_CAP: usize = 128;

/// 会话 → 已锁死的 hint（空串 = none 档，也缓存避免每轮重读设置）。
#[derive(Default)]
struct Resolved {
    map: HashMap<Option<String>, String>,
    order: VecDeque<Option<String>>,
}

/// 思考语言注入监听器（设置真源 = Arc<SettingsStore>；解析只发生在
/// 缓存未命中的首次请求，锁持有时间可忽略）。
pub struct ThinkingLanguageInject {
    settings: Arc<SettingsStore>,
    resolved: Mutex<Resolved>,
}

impl ThinkingLanguageInject {
    pub fn new(settings: Arc<SettingsStore>) -> Self {
        Self {
            settings,
            resolved: Mutex::new(Resolved::default()),
        }
    }

    /// 会话锁死语义的实现体：命中即用缓存，未命中解析并落缓存。
    fn hint_for(&self, session: &Option<String>) -> String {
        if let Ok(guard) = self.resolved.lock() {
            if let Some(hit) = guard.map.get(session) {
                return hit.clone();
            }
        }
        let hint = resolve(
            &self.settings.thinking_language(),
            &self.settings.locale(),
            &sys_locale(),
        )
        .to_string();
        if let Ok(mut guard) = self.resolved.lock() {
            if !guard.map.contains_key(session) {
                guard.order.push_back(session.clone());
                while guard.order.len() > CACHE_CAP {
                    if let Some(old) = guard.order.pop_front() {
                        guard.map.remove(&old);
                    }
                }
            }
            guard.map.insert(session.clone(), hint.clone());
        }
        hint
    }
}

/// 纯函数解析（可测）：思考语言档 × 界面语言 × OS 语言 → 提示词。
/// 空串 = 不注入；未知档值按缺省 follow_ui 语义回落。
fn resolve(language: &str, ui_locale: &str, os_locale: &str) -> &'static str {
    match language {
        THINKING_NONE => NONE_HINT,
        "zh-CN" => ZH_HINT,
        "en-US" => EN_HINT,
        // THINKING_FOLLOW_UI 与未知值一起回落跟随界面（缺省档语义）
        _ => {
            let zh = match ui_locale {
                "zh-CN" => true,
                "en" => false,
                _ => os_locale.starts_with("zh"),
            };
            if zh {
                ZH_HINT
            } else {
                EN_HINT
            }
        }
    }
}

/// OS 界面语言（"zh-CN" / "en-US" / …；取不到按 en）。
fn sys_locale() -> String {
    sys_locale::get_locale().unwrap_or_else(|| "en".into())
}

#[async_trait]
impl WaterfallListener for ThinkingLanguageInject {
    async fn handle(&self, key: &str, payload: &Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        let Ok(mut req) = serde_json::from_value::<RequestPayload>(payload.clone()) else {
            return WaterfallDecision::Continue;
        };
        let hint = self.hint_for(&req.session_id);
        if hint.is_empty() {
            return WaterfallDecision::Continue; // none 档：零参与
        }
        tracing::info!(
            session = ?req.session_id,
            hint_len = hint.len(),
            had_system = req.system.is_some(),
            "思考语言注入"
        );
        req.system = match req.system.take() {
            Some(prev) => Some(format!("{prev}\n\n{hint}")),
            None => Some(hint),
        };
        // 序列化失败按 fail-open 放行（与 Memory-Inject 同纪律）
        match serde_json::to_value(&req) {
            Ok(v) => WaterfallDecision::Replace(v),
            Err(_) => WaterfallDecision::Continue,
        }
    }
}

#[cfg(test)]
mod tests;
