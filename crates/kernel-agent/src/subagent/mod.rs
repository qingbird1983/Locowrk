//! 子 Agent 调度（⑮，DESIGN §7.2 扩展）：**宿主/内核侧**形态而非 C ABI
//! 插件——子 Agent 要递归跑内核循环，依赖方向禁止插件依赖 kernel-agent。
//!
//! v1 收敛为**单层串行**：`subagent_run` 工具面把一个任务交给子 Agent
//! （子会话 = 父转录前缀 fork 的内存投影，⑨ rewind/fork 同源语义），
//! 子跑完只把摘要回给父；子不可再生子（FilterToolSource 硬剔除）。
//!
//! 会话身份流转：Kernel 每步经 [`SubagentGate::note`] 刷新父转录快照
//! （键 = session_id，⑭ step_as 填充的身份），工具执行时取快照 fork；
//! 子审批冒泡同一审批门，`ToolCallReq.session_id` 携带子会话 id 作
//! 标识。快照条目随会话自然残留（进程生命周期，量级 = 转录本身），
//! 超 [`GATE_CAP`] 全清重建，不做精细驱逐。

mod filter;
mod run;
mod tool;

pub use filter::FilterToolSource;
pub use tool::SubagentTool;

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use session_store::Session;

/// 快照表防御上限：超过即全清（当前会话下一步会随 note 重新积累）。
const GATE_CAP: usize = 128;

/// 父转录快照表 + 子会话序号。Kernel 持有它（每步刷新），SubagentTool
/// 执行时读取——工具与内核之间唯一的耦合点，纯数据无回引用。
#[derive(Default)]
pub struct SubagentGate {
    parents: Mutex<HashMap<String, Arc<Session>>>,
    counter: AtomicU64,
}

impl SubagentGate {
    /// 刷新某会话的可见前缀快照（step_inner 每步调用；覆盖式插入）。
    pub(crate) fn note(&self, session_id: &str, session: &Session) {
        let mut map = self.parents.lock().expect("subagent gate poisoned");
        if map.len() >= GATE_CAP && !map.contains_key(session_id) {
            map.clear();
        }
        map.insert(
            session_id.to_string(),
            Arc::new(session.fork(session.cursor())),
        );
    }

    /// 取父转录前缀（工具执行时）。缺席 = 会话没在本内核走过 step
    /// （如组装点忘了 with_subagent），子 Agent 无从 fork，如实报错。
    pub(crate) fn snapshot(&self, session_id: &str) -> Option<Arc<Session>> {
        self.parents
            .lock()
            .expect("subagent gate poisoned")
            .get(session_id)
            .cloned()
    }

    /// 子会话 id：`{parent}-sub-{n}`，进程内唯一（审批标识/日志检索用）。
    pub(crate) fn next_child_id(&self, parent_session_id: &str) -> String {
        let n = self.counter.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{parent_session_id}-sub-{n}")
    }
}
