//! 阶段四 demo 的"审批插件"：一个进程内组件扮演两个角色（§14 验收）——
//! 1. waterfall 决策环：在 `agent/request` 改写 `params.mode`（策略参数注入）
//! 2. ApprovalResolver：首次不可逆调用拒绝，之后放行

use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use cordis_core::{WaterfallDecision, WaterfallListener};
use kernel_agent::{ApprovalDecision, ApprovalResolver, ToolCallReq, points};
use plugin_api::ToolSpec;
use serde_json::{Value, json};

pub struct ApprovalPlugin {
    /// 首次审批拒绝（演示 denied 路径），之后放行。
    deny_used: AtomicBool,
}

impl ApprovalPlugin {
    pub fn new() -> Self {
        Self {
            deny_used: AtomicBool::new(false),
        }
    }
}

impl Default for ApprovalPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WaterfallListener for ApprovalPlugin {
    async fn handle(&self, key: &str, payload: &mut Value) -> WaterfallDecision {
        if key != points::REQUEST {
            return WaterfallDecision::Continue;
        }
        // 故意保持 `clone → 改 → Replace` 的**外部决策方形状**（台账 T2 之后内核七个
        // 注入环已改为就地写 + Continue）。这一环是 Replace 语义在本仓唯一的真实
        // 使用者：留着它，`Replace` 就不是"只有测试在用"的死分支。
        let mut p = payload.clone();
        // 策略注入：强制 echo 用 upper 模式（对下游 LLM 脚本可见）
        if let Some(obj) = p.as_object_mut() {
            let params = obj.entry("params").or_insert(json!({}));
            if let Some(pp) = params.as_object_mut() {
                pp.insert("mode".into(), json!("upper"));
            }
        }
        println!("  [approval-plugin] agent/request: params.mode -> upper");
        WaterfallDecision::Replace(p)
    }
}

#[async_trait]
impl ApprovalResolver for ApprovalPlugin {
    async fn resolve(&self, req: &ToolCallReq, _spec: &ToolSpec) -> ApprovalDecision {
        let first = !self.deny_used.swap(true, Ordering::Relaxed);
        if first {
            println!(
                "  [approval-plugin] 拒绝 {}（首次不可逆调用需人工复核）",
                req.tool
            );
            ApprovalDecision::deny("first irreversible call needs human review")
        } else {
            println!("  [approval-plugin] 批准 {}", req.tool);
            ApprovalDecision::allow()
        }
    }
}
