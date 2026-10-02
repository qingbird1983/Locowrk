//! 会话 → 项目根解析（㉘，项目级记忆隔离的宿主侧"单一真源"）。
//!
//! 权威数据 = 会话旁车 `<id>.meta.json` 的 `workspace` 字段（§2.3 关联，
//! v2.50 落地）。desktop 三处消费同一份解析器：
//! ① `plugin_host::set_session_workspace_resolver`——FFI 调用期上下文的
//!    workspace 来源（memory-sqlite 等插件经 vtable `session_context`
//!    反向查询，把写入/读取按项目隔离）；
//! ② `MemoryStore` 注入召回的按键位过滤（kernel `MemorySource` 只给
//!    session_id，workspace 是宿主概念）；
//! ③ `ExperienceIndex` 经历召回的同款过滤。
//!
//! 未归属 / 旁车缺失 / id 非法 → `None`（消费方统一映射为未归属键位
//! `""`，绝不 panic、绝不把别的项目数据漏进来）。子 Agent 会话没有旁车
//! （⑮ 黑盒），解析为未归属——已知边界：子内记忆落在未归属键位，不
//! 随父项目走（诚实且安全，跨项目零泄漏）。

use std::path::PathBuf;
use std::sync::Arc;

use session_store::read_meta;

/// 解析器类型：session_id → 该会话归属的项目根（未归属 = None）。
pub type WorkspaceResolver = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// 组装点构造：绑死会话目录（`<数据根>/sessions/`）。`read_meta` 内部
/// 走 session id 白名单，路径穿越在源头挡掉。
pub fn session_workspace_resolver(sessions_dir: PathBuf) -> WorkspaceResolver {
    Arc::new(move |session_id: &str| read_meta(&sessions_dir, session_id).and_then(|m| m.workspace))
}

/// 键位归一：消费方统一入口——`None`（无会话/未归属）与 `Some("")` 都
/// 落到未归属键位。sqlite 侧 `workspace` 列 NOT NULL DEFAULT ''。
pub fn scope_key(workspace: Option<&str>) -> String {
    workspace
        .filter(|w| !w.is_empty())
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests;
