//! Waterfall 三决策点（DESIGN.md §7.2）：`agent/pre-step`（处理用户消息前）、
//! `agent/request`（发 LLM 请求前）、`agent/request-error`（请求失败后）。
//!
//! payload 是带 schema 的 JSON——与 C ABI 同款纪律：决策方与内核解耦，
//! 只共享 JSON 形状。插件经 `ctx.events.on_waterfall` 注册决策环。

use std::time::Duration;

use cordis_core::{RejectReason, WaterfallDecision};
use plugin_api::ToolSpec;
use serde::{Deserialize, Serialize};
use session_store::Message;

use crate::CompactionOutcome;

/// 决策点事件名（§3.5）。
pub mod points {
    /// 处理用户消息前：插件可拒绝/改写消息。
    pub const PRE_STEP: &str = "agent/pre-step";
    /// 发 LLM 请求前：插件可改 provider/model/参数。
    pub const REQUEST: &str = "agent/request";
    /// 请求失败后：插件可决定 Retry。
    pub const REQUEST_ERROR: &str = "agent/request-error";
}

/// `agent/pre-step` 的 payload。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PreStepPayload {
    pub messages: Vec<Message>,
}

/// `agent/request` 的 payload。`model` / `params` 可被决策环改写——
/// 审批插件通过该点注入策略参数（§14 阶段四验收）。`tools` 是工具目录
/// 快照（LLM.md：适配层负责映射各家格式），`serde(default)` 保持与
/// 旧决策方互通。`compacted` 是压缩结果的带回位（⑨）：kernel 据此落
/// `ContextCompacted` 事件进会话日志；写入方 ctx-compact 已随审计修复批 C
/// 删除，字段本身是留给任何 `agent/request` 决策方的可扩展契约，且历史
/// 事件重放在读它的形状。`system` 是请求级系统提示（⑪）：
/// Memory-Inject 把持久记忆写在这里——它在消息列表之外，任何改写消息的
/// 决策方都只动 `messages`，"系统提示永不压缩"（Context 文档 §5.3）由结构
/// 保证，会话日志也不落注入内容。`session_id`（㉘）：本 step 的会话 id——
/// Memory-Inject 交给宿主检索源做项目隔离（workspace 是宿主概念，内核
/// 只透传 id，不解释）。`tree_directory`（M4 记忆树切流）：**Some = 树
/// 会话**（含空串 = 尚无捆）——内核组装侧已把 `messages` 切到未整理尾巴，
/// 本字段携带 L0 目录文本，链上 TreeDirectoryInject 把它追加到 system
/// 尾部（Memory-Inject 之后 = 目录在记忆块后，树 v2 §7.1 组装次序）；
/// None = legacy 会话，组装零变化。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestPayload {
    pub messages: Vec<Message>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub params: serde_json::Value,
    #[serde(default)]
    pub tools: Vec<ToolSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compacted: Option<CompactionOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tree_directory: Option<String>,
}

/// `agent/request` 决策环的**唯一取还形状**（台账 T2，2026-10-18）：载荷 move 进环，
/// 改完在 `Drop` 里 move 回 payload——整条链上不再有任何整包克隆。
///
/// 改前的形状是七个注入环各写一遍
/// `let Ok(mut req) = from_value::<RequestPayload>(payload.clone()) else { Continue }`，
/// 收完再 `replace_decision(req)`（= `to_value`）还回去。`agent/request` 上**每一环都要改写**，
/// 于是 event_bus 当年那句"只让改写者付克隆"变成"每次请求付七份深拷贝"，
/// 而一份 = 全部对话历史 + 工具目录。
///
/// **为什么是 RAII 守卫，而不是 `take()` + `put()` 两个函数**：这些环各有十几个早退分支
/// （legacy 会话零参与 / 无指令文件 / 无碎片 / 消息列为空 / 检索为空……），
/// 每个早退都必须先把载荷还回去；手写 `put` 漏掉任何一处，就是把 `Value::Null`
/// 交给下一环——那种错误只在真实会话里出现，单测很难穷举。守卫让"还"不可能忘。
/// 先例 = `desktop/src-tauri/src/stop.rs:63-84` 的注销守卫。
///
/// `pub` 而不是 `pub(crate)`：宿主侧的注入环（`desktop/src-tauri/src/thinking_language.rs`）
/// 用同一形状，内核内外各写一份就必然漂——它们判的是同一件事"这份 payload 是不是 request"。
pub struct RequestGuard<'a> {
    payload: &'a mut serde_json::Value,
    req: RequestPayload,
}

impl Drop for RequestGuard<'_> {
    fn drop(&mut self) {
        // 这一步实际不会失败：RequestPayload 整棵树都是 JSON 原生值
        // （Vec<Message> / Vec<ToolSpec> / serde_json::Value 的 Serialize 都不产错误）。
        // 真失败就把 Null 留在原地让出环解析报错——panic 会穿过 FFI 与 spawn_blocking
        // （内核纪律 4），比一份坏载荷更糟。
        *self.payload = serde_json::to_value(&self.req).unwrap_or(serde_json::Value::Null);
    }
}

impl RequestGuard<'_> {
    /// 环内取载荷引用的**唯一入口**：`let req = g.req();` 取一次，之后读写都走这个裸引用。
    ///
    /// 不提供 `Deref`/`DerefMut`：每次自动解引用都是对守卫整体的一次借用，字段访问
    /// 无法被拆成 disjoint borrow——`fragments.rs` 那种"先 `messages.last_mut()` 拿到
    /// `&mut String`、随后还要读 `session_id`"的现役形状会直接拒编。裸 `&mut RequestPayload`
    /// 上的字段路径才是编译器能分辨的。
    pub fn req(&mut self) -> &mut RequestPayload {
        &mut self.req
    }
}

/// 取载荷（解析失败返回终态决策，调用方直接 `return` 它）。`by` = 环名，只进错误文案。
///
/// 坏载荷的行为差异要如实说清：原形状那个 `else { Continue }` 被注释称作 fail-open，
/// 但它**放不出去**——出环（`kernel/request.rs` 的 `waterfall_request`）对 Continue 走的
/// 正是 `from_value(out.payload)?`，同一份坏载荷在下一行就变成 `KernelError`。
/// 所以这里不是"放行改成拒绝"，而是**同一个失败提前发生、且点名了是哪个环先发现的**，
/// 省下后面几环各白跑一遍（现网形状下 = 六次整包 clone）。
pub fn take_request<'a>(
    payload: &'a mut serde_json::Value,
    by: &str,
) -> Result<RequestGuard<'a>, WaterfallDecision> {
    let owned = std::mem::replace(payload, serde_json::Value::Null);
    serde_json::from_value::<RequestPayload>(owned)
        .map(|req| RequestGuard { payload, req })
        .map_err(|e| {
            WaterfallDecision::Reject(RejectReason {
                by: by.to_string(),
                reason: format!("agent/request 的载荷解析不了（发出方与决策环不同形？）：{e}"),
            })
        })
}

/// 追加而非覆盖：`system` / 注入块的唯一合并口（台账 T2 收口）。
///
/// 现值 `None` = 整段就是新值；`Some` = `\n\n` 接尾；`add` 为 `None` = 不动。
///
/// 改前它有四份实现：memory_inject 私有的 `Self::append_block`（`Option` 进 `Option` 出）、
/// base_system / tree_context / thinking_language 各一份手写的
/// `req.system = match req.system.take() { Some(prev) => format!("{prev}\n\n{seg}"), None => seg }`。
/// 四份语义逐字相同，只形状不同。收成一处不只是洁癖：T2 之后环握的是 `&mut RequestPayload`，
/// "读同一字段再写同一字段"那个形状在可变引用上会撞 borrowck（owned 值不会），
/// 每一处都必须改写——各写一种绕法必然漂。
///
/// "追加"本身是纪律不是风格：kernel 每 step 重建 payload 时 `system` 恒为 None，
/// 但任何决策方都可能先写它（思考语言在记忆块之后、目录又在最后），覆盖 = 抹掉前一环。
pub fn append_block(system: &mut Option<String>, add: Option<&str>) {
    let Some(add) = add else { return };
    match system {
        Some(prev) => {
            prev.push_str("\n\n");
            prev.push_str(add);
        }
        slot @ None => *slot = Some(add.to_string()),
    }
}

/// `agent/request-error` 的 payload。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RequestErrorPayload {
    pub error: String,
}

/// `agent/request-error` 返回 Retry 决策时的重试预算（§7.2）：同一 step 内
/// 累计次数与累计延迟上限，超限转为失败，防止决策点形成无限循环。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryBudget {
    pub max_retries: u32,
    pub max_total_delay: Duration,
}

impl Default for RetryBudget {
    fn default() -> Self {
        Self {
            max_retries: 3,
            max_total_delay: Duration::from_secs(10),
        }
    }
}
