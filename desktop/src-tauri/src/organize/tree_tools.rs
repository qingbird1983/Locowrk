//! memory_expand / memory_collapse（M4 惰性加载，树 v2 §8；UI shell
//! §3.38③④）：宿主内置工具面（schedule 三件套同款归宿主先例——树投影
//! 是宿主资产）。经 `KernelToolSource::with_builtin` 进目录；**会话门控
//! 双保险**：目录层（tool_source.rs 树会话才列）+ 调用层（本文件 resolve
//! 语义，legacy 会话调到即如实失败）。
//!
//! expand = Pure（读树 + 读盘切片，"展开"只是 bookkeeping——expanded_at
//! 进投影文件，不碰用户数据），exposure = Always（树会话目录常驻，§8.2）；
//! collapse = Compensable（收起改写未来前缀 = 计划外缓存失效，§8.3 裁定
//! 缺省不暴露，设置开关才进目录）——它省窗口空间不省钱，当省钱工具用是
//! §16 头号误区。

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{json, Value};

use memory_tree::{build_items, transcript, TreeStore};

use crate::workspace::WorkspaceResolver;

static VERSION: std::sync::LazyLock<semver::Version> =
    std::sync::LazyLock::new(|| semver::Version::new(1, 0, 0));

fn unavailable(what: &str, why: &str) -> ToolError {
    ToolError::NotAvailable(format!("{what} 不可用：{why}"))
}

fn failed(tool: &str, e: String) -> ToolError {
    ToolError::Failed {
        tool: tool.into(),
        message: e,
    }
}

// ---- memory_expand ----

fn expand_spec() -> &'static ToolSpec {
    static SPEC: OnceLock<ToolSpec> = OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "memory_expand".into(),
        description: "Expand a chapter of this session's memory tree — older turns that were \
                      organized out of the visible context. Pass the chapter_id from the table \
                      of contents. level=1 returns the chapter's summary (~200 tokens); level=2 \
                      returns its full original transcript. The result is appended to the \
                      conversation and stays visible."
            .into(),
        schema: json!({
            "type": "object",
            "properties": {
                "chapter_id": { "type": "string", "description": "chapter id from the memory tree directory (e.g. \"b2\")" },
                "level": { "type": "integer", "minimum": 1, "maximum": 2,
                           "description": "1=summary, 2=full transcript" }
            },
            "required": ["chapter_id", "level"]
        }),
        side_effect: SideEffect::Pure,
        exposure: Exposure::Always,
    })
}

pub struct MemoryExpandTool {
    tree_store: TreeStore,
    workspace_of: WorkspaceResolver,
    sessions_dir: PathBuf,
}

impl MemoryExpandTool {
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

    fn load_tree(
        &self,
        workspace: &str,
        session_id: &str,
    ) -> Result<memory_tree::MemoryTree, String> {
        Ok(self
            .tree_store
            .load(workspace, session_id)
            .unwrap_or_else(|| memory_tree::MemoryTree::new(workspace, session_id)))
    }
}

#[async_trait]
impl Tool for MemoryExpandTool {
    fn spec(&self) -> &ToolSpec {
        expand_spec()
    }

    fn plugin_version(&self) -> &semver::Version {
        &VERSION
    }

    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let session_id = ctx
            .session_id
            .clone()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| unavailable("memory_expand", "无会话身份"))?;
        let chapter_id = args
            .get("chapter_id")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| failed("memory_expand", "缺少 chapter_id".into()))?;
        let level = args.get("level").and_then(Value::as_u64).unwrap_or(1);
        if !(1..=2).contains(&level) {
            return Err(failed(
                "memory_expand",
                format!("level {level} 非法（1=摘要 2=原文）"),
            ));
        }
        let workspace = (self.workspace_of)(&session_id).unwrap_or_default();
        let tree = self
            .load_tree(&workspace, &session_id)
            .map_err(|e| failed("memory_expand", e))?;
        let chapter = tree
            .chapters
            .get(chapter_id)
            .filter(|b| !b.pending)
            .ok_or_else(|| {
                failed(
                    "memory_expand",
                    format!("章节 {chapter_id} 不存在（或整理在途）"),
                )
            })?;

        let content = if level == 1 {
            json!({
                "chapter_id": chapter_id,
                "title": chapter.title,
                "summary": chapter.summary,
            })
        } else {
            // level=2：原文切片（M3 view 同源：磁盘日志按 event_range 切，
            // 材料行同形转文本）
            let path = session_store::session_file(&self.sessions_dir, &session_id)
                .map_err(|e| failed("memory_expand", e.to_string()))?;
            let (session, _) = session_store::load_session(&path)
                .map_err(|e| failed("memory_expand", format!("会话日志不可读: {e}")))?;
            let events = session.events();
            let (from, to) = chapter.event_range;
            if from >= events.len() {
                return Err(failed(
                    "memory_expand",
                    format!("章节 {chapter_id} 区间越界（会话日志已截断？）"),
                ));
            }
            let end = (to + 1).min(events.len());
            let items = build_items(&events[from..end], from);
            json!({
                "chapter_id": chapter_id,
                "title": chapter.title,
                "transcript": transcript(&items),
            })
        };

        // 展开记账（解除收起 §8.3；写投影失败只影响自动收起的挑选，不拦读）
        let mut tree = tree;
        tree.mark_expanded(chapter_id, now_ms());
        if let Err(e) = self.tree_store.save(&tree) {
            tracing::warn!("expanded_at 记账失败: {e}");
        }
        Ok(content)
    }
}

// ---- memory_collapse ----

fn collapse_spec() -> &'static ToolSpec {
    static SPEC: OnceLock<ToolSpec> = OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "memory_collapse".into(),
        description: "Collapse an expanded chapter: its previously expanded content is replaced \
                      by a stub in the conversation to free context window space. NOTE: this \
                      rewrites the prompt prefix (a planned cache miss) — use it to relieve \
                      window pressure, not to save cost. The chapter stays listed in the memory \
                      tree; call memory_expand to bring it back."
            .into(),
        schema: json!({
            "type": "object",
            "properties": {
                "chapter_id": { "type": "string", "description": "chapter id previously expanded" }
            },
            "required": ["chapter_id"]
        }),
        side_effect: SideEffect::Compensable {
            undo_tool: "memory_expand".into(),
        },
        exposure: Exposure::Hidden,
    })
}

pub struct MemoryCollapseTool {
    tree_store: TreeStore,
    workspace_of: WorkspaceResolver,
}

impl MemoryCollapseTool {
    pub fn new(tree_store: TreeStore, workspace_of: WorkspaceResolver) -> Arc<Self> {
        Arc::new(Self {
            tree_store,
            workspace_of,
        })
    }
}

#[async_trait]
impl Tool for MemoryCollapseTool {
    fn spec(&self) -> &ToolSpec {
        collapse_spec()
    }

    fn plugin_version(&self) -> &semver::Version {
        &VERSION
    }

    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let session_id = ctx
            .session_id
            .clone()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| unavailable("memory_collapse", "无会话身份"))?;
        let chapter_id = args
            .get("chapter_id")
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| failed("memory_collapse", "缺少 chapter_id".into()))?;
        let workspace = (self.workspace_of)(&session_id).unwrap_or_default();
        let mut tree = self
            .tree_store
            .load(&workspace, &session_id)
            .unwrap_or_else(|| memory_tree::MemoryTree::new(&workspace, &session_id));
        if !tree.mark_collapsed(chapter_id, now_ms()) {
            return Err(failed(
                "memory_collapse",
                format!("章节 {chapter_id} 不存在或从未展开（无可收内容）"),
            ));
        }
        self.tree_store
            .save(&tree)
            .map_err(|e| failed("memory_collapse", e))?;
        Ok(json!({
            "collapsed": chapter_id,
            "note": "chapter content replaced by a stub in context; memory_expand brings it back"
        }))
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests;
