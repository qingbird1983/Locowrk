//! FragmentInject（提示词分层设计 §4.4）：`agent/request` waterfall
//! 监听器——把「会话态碎片」（整理完成 / Plan 模式 / 轮次到顶）以
//! `<system-reminder>` 包裹拼在**最后一条 user 消息**尾部。
//!
//! 落点裁决（设计稿 §6 二选一已查证）：kernel waterfall 的 payload 携带
//! 完整 `messages` 且决策环拿的是它的可变引用（台账 T2 之前是"克隆后
//! Replace 整包换入"，旧 ctx-compact 改写消息走的就是这条通路）——内核
//! 中间件可落，headless 同享。pending 表归宿主：内核定
//! [`FragmentSource`] 契约、组装点实现（MemorySource 同款模式）——碎片
//! 事件全是桌面面（整理收束/权限切换/步数到顶），内核不解释。
//!
//! 三拍语义（设计稿 §6 测试锚点）：事件处置位 → 下一次**轮开场**请求
//! 拼接出现一次 → pending 清空（同轮后续 step 与下一轮不再携带）。
//! 「轮开场判据 = 最后一条消息是 User」零额外状态：事件若在轮中发生
//! （如运行中切 Plan），碎片留在表里等下一轮开场——轮中改写历史消息
//! 会打断本轮请求的前缀，不做。
//!
//! 缓存代价（记档，设计稿 §9.5 同口径）：拼接清标记后，同轮后续 step
//! 的请求在该条 user 消息处与前一步分叉（碎片消失）——其后请求体本就
//! 是新增内容，净损失 ≈ 一条 user 消息的编码，可接受。

use std::sync::Arc;

use async_trait::async_trait;
use cordis_core::{WaterfallDecision, WaterfallListener};
use serde_json::Value;
use session_store::Message;

use crate::waterfall::{points, take_request};

/// 会话态碎片源契约：宿主实现持有 pending 表（按会话键位）。`take`
/// = 取走并清空（drain）；无身份（`None`）返回空表——碎片从不进
/// headless / 无归属请求。
pub trait FragmentSource: Send + Sync + 'static {
    fn take(&self, session_id: Option<&str>) -> Vec<String>;
}

/// `<system-reminder>` 包裹（集中一处，防注入）：内文里的字面闭合标签
/// 转义（ZCode system-reminder 注册表同款思路）——文件正文、事件文案
/// 永远成不了标签边界。指令文件注入共用本件。
pub fn wrap_system_reminder(inner: &str) -> String {
    let escaped = inner.replace("</system-reminder>", "<\\/system-reminder>");
    format!("<system-reminder>\n{escaped}\n</system-reminder>")
}

/// 碎片注入监听器。
pub struct FragmentInject {
    source: Arc<dyn FragmentSource>,
}

impl FragmentInject {
    pub fn new(source: Arc<dyn FragmentSource>) -> Self {
        Self { source }
    }
}

#[async_trait]
impl WaterfallListener for FragmentInject {
    async fn handle(&self, key: &str, payload: &mut Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        let mut guard = match take_request(payload, "FragmentInject") {
            Ok(g) => g,
            Err(d) => return d,
        };
        let req = guard.req();
        // 轮开场判据：最后一条消息是 User 才取件拼接——轮中 step 不改
        // 历史消息（前缀纪律），碎片留在 pending 表等下一轮开场。
        let Some(Message::User { content, .. }) = req.messages.last_mut() else {
            return WaterfallDecision::Continue;
        };
        let frags = self.source.take(req.session_id.as_deref());
        if frags.is_empty() {
            return WaterfallDecision::Continue;
        }
        // 多碎片同轮密集触发 = 合并一条多段 reminder（设计稿 §9.4）
        let reminder = wrap_system_reminder(&frags.join("\n\n"));
        content.push_str("\n\n");
        content.push_str(&reminder);
        WaterfallDecision::Continue
    }
}

#[cfg(test)]
mod tests;
