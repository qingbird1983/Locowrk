//! TreeDirectoryInject（M4 记忆树切流，树 v2 §7.1）：`agent/request`
//! waterfall 监听器——把内核组装侧交来的「记忆树目录」（`RequestPayload::
//! tree_directory`）追加到 payload.system 尾部。
//!
//! 链位：ToolExposure → Memory-Inject → **TreeDirectoryInject** →
//! ctx-compact——目录排在记忆块之后（组装次序 [2]记忆 → [3]目录 → [4]
//! 尾巴，树 v2 §7.1），system 前缀（静态 system + 记忆块）不受目录增长
//! 影响（前缀缓存只看字节前缀，目录追加发生在块尾）。
//!
//! 目录文本由宿主 [`TreeContextSource`](crate::TreeContextSource) 生成，
//! 本件只做追加：空串不追加（Some("") = 树会话尚无捆——marker 仍随
//! payload 走，ctx-compact 据此跳过）。kernel 每 step 重建 payload，
//! `tree_directory` 恒从组装侧带来，无重复追加问题。

use async_trait::async_trait;
use cordis_core::{WaterfallDecision, WaterfallListener};
use serde_json::Value;

use crate::waterfall::{RequestPayload, points};

/// 目录注入监听器（无状态——所有会话态都在 payload 里）。
pub struct TreeDirectoryInject;

#[async_trait]
impl WaterfallListener for TreeDirectoryInject {
    async fn handle(&self, key: &str, payload: &Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        let Ok(mut req) = serde_json::from_value::<RequestPayload>(payload.clone()) else {
            return WaterfallDecision::Continue;
        };
        let Some(dir) = req.tree_directory.clone() else {
            return WaterfallDecision::Continue; // legacy 会话：零参与
        };
        if !dir.is_empty() {
            req.system = match req.system.take() {
                Some(prev) => Some(format!("{prev}\n\n{dir}")),
                None => Some(dir),
            };
        }
        // to_value 失败按 fail-open 放行（与 Memory-Inject 同纪律）
        super::replace_decision(req)
    }
}
