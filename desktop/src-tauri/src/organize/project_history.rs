//! memory_project_history（v2.142，《Cross-Session Recall Design v1》§2.3）：
//! 项目时间线 =「之前做了什么」的**第一答案面**（用户裁定：最先查到的
//! 应该是时间线里的**总结 + 该总结所包含的标题**，这就够了；细节再
//! memory_expand 按 (session_id, chapter_id) 下钻）。
//!
//! 数据源与 view.rs `tree_project` 同源（scan_workspace 全会话 + 旁车标题
//! 映射），但工具不依赖 Organizer（组装点在 Organizer 构建之前）——自带
//! 同款聚合。`Pure` + **`Always`**（Q2 · MR §4.3 起，原为 OnDemand）：读侧零风险，
//! 计划模式围栏按 `side_effect` 放行（**与 exposure 无关**，改档不影响围栏）；
//! TreeToolGate 无需改（只管 collapse）。提到 Always 的理由见 `spec()` 处注释。
//!
//! 钳制：最近 [`MAX_SESSIONS`] 个会话（session_id 升序取尾 = 时间取尾；
//! 桌面 id 含毫秒时间戳，字典序即时间序），超出置 truncated 指路
//! memory_search_titles。标题不带 L1 正文（裁定：总结+标题够了，控制
//! 体积；L1 走 expand level=1）。未归属会话（无 workspace）天然不在
//! scan_workspace 结果里（v2.59 严格口径一致）。

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{json, Value};

use memory_tree::TreeStore;

use crate::workspace::WorkspaceResolver;

static VERSION: std::sync::LazyLock<semver::Version> =
    std::sync::LazyLock::new(|| semver::Version::new(1, 0, 0));

/// 时间线常驻体积护栏：最多返回最近 20 个会话。
const MAX_SESSIONS: usize = 20;

fn spec() -> &'static ToolSpec {
    static SPEC: OnceLock<ToolSpec> = OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "memory_project_history".into(),
        description: "Project memory timeline across ALL sessions of the current project, \
                      oldest first. THE first place to look when the user asks what was done \
                      before, project history, or past decisions: returns each session (its \
                      title) with the summary written at each organize round and the chapter \
                      titles that round covers. For details of a chapter, call memory_expand \
                      with its (session_id, chapter_id). Only the most recent 20 sessions are \
                      listed — use memory_search_titles to find older topics."
            .into(),
        schema: json!({ "type": "object", "properties": {} }),
        side_effect: SideEffect::Pure,
        // Q2（MR 稿 §4.3）：`OnDemand → Always`。它是「用户问过去做了什么」这条
        // 动线的**入口工具**，而读侧是本地 JSON 聚合（零网络、零模型调用）；留在
        // OnDemand 的实测后果 = MR 稿 §0-2：新会话零会话内用量 + embedding 未配置
        // ⇒ 语义相似度退化 ⇒ 被 Top-20 截断**挤出目录**，模型根本不知道它存在。
        // 代价 = Always 名额约 14 → 15（OnDemand 档 6 → 5），`max_tools` 是设置项，
        // 名额紧张时由用户自己调。
        exposure: Exposure::Always,
    })
}

pub struct ProjectHistoryTool {
    tree_store: TreeStore,
    workspace_of: WorkspaceResolver,
    /// 旁车标题读取（会话行带人类可读标题）。
    sessions_dir: PathBuf,
}

impl ProjectHistoryTool {
    pub fn new(
        tree_store: TreeStore,
        workspace_of: WorkspaceResolver,
        sessions_dir: PathBuf,
    ) -> Arc<Self> {
        Arc::new(Self {
            tree_store,
            workspace_of,
            sessions_dir,
        })
    }
}

#[async_trait]
impl Tool for ProjectHistoryTool {
    fn spec(&self) -> &ToolSpec {
        spec()
    }

    fn plugin_version(&self) -> &semver::Version {
        &VERSION
    }

    async fn call(&self, ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        let session_id = ctx
            .session_id
            .clone()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                ToolError::NotAvailable("memory_project_history 不可用：无会话身份".into())
            })?;
        let workspace = (self.workspace_of)(&session_id).unwrap_or_default();
        let session_titles: std::collections::HashMap<String, String> =
            crate::session_log::list(&self.sessions_dir)
                .into_iter()
                .filter_map(|m| m.title.map(|t| (m.id, t)))
                .collect();

        // scan_workspace 按 session_id 升序（桌面 id 含毫秒 = 时间正序）；
        // 超钳制取尾（最近优先）。
        let trees = self.tree_store.scan_workspace(&workspace);
        let total = trees.len();
        let truncated = total > MAX_SESSIONS;
        let start = total.saturating_sub(MAX_SESSIONS);

        let sessions: Vec<Value> = trees[start..]
            .iter()
            .map(|(tree, sid, _)| {
                let mut chapters: Vec<&memory_tree::MemoryChapter> =
                    tree.chapters.values().filter(|b| !b.pending).collect();
                // BTreeMap 字典序（b10 < b2）→ 章节序号数值排（创建序）。
                chapters.sort_by_key(|b| {
                    b.chapter_id
                        .0
                        .strip_prefix('b')
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(u64::MAX)
                });
                // 章节区间 ⊆ 总结区间即归属该轮（recorder 材料与总结同轮
                // 切片，包含关系成立）；无总结覆盖 = other（v2.141 前存量）。
                let chapter_json = |b: &memory_tree::MemoryChapter| {
                    json!({ "id": b.chapter_id.0, "title": b.title })
                };
                let mut other = Vec::new();
                let summaries: Vec<Value> = tree
                    .summaries
                    .iter()
                    .map(|s| {
                        let covered: Vec<Value> = chapters
                            .iter()
                            .filter(|b| b.event_range.0 >= s.from && b.event_range.1 <= s.to)
                            .map(|b| chapter_json(b))
                            .collect();
                        json!({ "text": s.text, "chapters": covered })
                    })
                    .collect();
                for b in &chapters {
                    let covered = tree.summaries.iter().any(|s| {
                        b.event_range.0 >= s.from && b.event_range.1 <= s.to
                    });
                    if !covered {
                        other.push(chapter_json(b));
                    }
                }
                json!({
                    "session_id": sid,
                    "session_title": session_titles.get(sid).cloned().unwrap_or_default(),
                    "summaries": summaries,
                    "other_chapters": other,
                })
            })
            .collect();

        let mut out = json!({
            "workspace": workspace,
            "total_sessions": total,
            "truncated": truncated,
            "sessions": sessions,
        });
        if truncated {
            out["note"] =
                json!("older sessions omitted; use memory_search_titles to find older topics");
        }
        Ok(out)
    }
}
