//! 子代理模型面（批 F）：子内核作用域的 LlmService 注入契约。组装点
//! （desktop）每子 run 调一次工厂造出「带回退链的服务 + 运行报告」对；
//! 内核只认这两个类型，不知道回退/钉档语义（§7.2 内核定契约、组装点实
//! 现，与 ToolSource 同模式）。

use std::sync::{Arc, Mutex};

use crate::kernel::LlmService;

/// 一次子 run 的模型使用报告：`SubagentLlm` 在路由切换时写入，
/// `run_child` 收束时读出并进 `SubagentOutcome`（model / model_failures）。
#[derive(Default)]
pub struct ChildModelReport {
    inner: Mutex<ReportInner>,
}

#[derive(Default)]
struct ReportInner {
    /// 实际用过的模型（provider/model，按首次使用序）。
    used: Vec<String>,
    /// 失败/跳过记因（含窗口装不下跳过的候选）。
    failures: Vec<String>,
}

impl ChildModelReport {
    /// 实际用过的模型（首次使用序去重）。组装点实现（SubagentLlm）写。
    pub fn record_used(&self, identity: &str) {
        let mut r = self.inner.lock().expect("child model report poisoned");
        if !r.used.iter().any(|u| u == identity) {
            r.used.push(identity.to_string());
        }
    }

    /// 失败/跳过记因（含窗口装不下跳过的候选）。组装点实现写。
    pub fn record_failure(&self, identity: &str, reason: &str) {
        self.inner
            .lock()
            .expect("child model report poisoned")
            .failures
            .push(format!("{identity}: {reason}"));
    }

    /// 收束快照：（末用模型，失败记因列表）。内核 run_child 读。
    pub(crate) fn snapshot(&self) -> (Option<String>, Vec<String>) {
        let r = self.inner.lock().expect("child model report poisoned");
        (r.used.last().cloned(), r.failures.clone())
    }
}

/// 工厂产物：服务 + 报告句柄（同一次构造，报告归 run_child 读）。
pub struct ChildModelHandle {
    pub service: Arc<dyn LlmService>,
    pub report: Arc<ChildModelReport>,
}

/// 每子 run 调一次（SubagentTool 组装点注入；None = 无子代理模型面，
/// 子内核回退全局 LlmService——⑮ 以来行为逐字节不变）。
pub type ChildModelFactory = Arc<dyn Fn() -> ChildModelHandle + Send + Sync>;
