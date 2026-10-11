//! memory_search_titles（M6.3，树 v2 §10.2「L0 标题模糊匹配」）：对当前
//! 项目**跨会话**的全部捆做 top-5 匹配——先有结构（树），再用向量辅助
//! 查找。v2.142 索引扩容：打分文本 = 标题 + L1 摘要 + 所在轮总结正文
//! （章节区间落在某 RunSummary [from,to] 内即归属该轮）——v2.141 总结
//! 是浓缩叙事，宽查询（「之前做了什么」）对短标题打分极易空手，总结
//! 正文才是语义主体；关键词降级同扩。
//!
//! 双模式如实：embedding 配置好 = 语义相似（cosine top-5，match_mode:
//! "semantic"）；缺席/失败 = 关键词 contains（match_mode: "keyword"），
//! 不谎报语义命中。候选池即 M6.4 项目联合视图同一数据源（scan_workspace
//! 全会话）。Pure + on_demand：读侧工具零风险；目录已在 system 尾，本
//! 工具补「目录太长 / 跨会话找」的场景（search_tools 可发现）。
//!
//! 会话门控双保险（memory_expand 同款）：目录层 tree 会话才列 + 调用层
//! resolve 语义（legacy 会话调到即如实失败）。

use std::sync::Arc;

use async_trait::async_trait;
use kernel_agent::{cosine, EmbedPort};
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use serde_json::{json, Value};

use memory_tree::TreeStore;

use crate::text::clip_ellipsis;
use crate::workspace::WorkspaceResolver;

static VERSION: std::sync::LazyLock<semver::Version> =
    std::sync::LazyLock::new(|| semver::Version::new(1, 1, 0));

const TOP_K: usize = 5;
/// 摘要跟随标题返回的截断（工具结果粒度=标题，摘要给 160 字符够判断相关性）。
const SUMMARY_CLIP: usize = 160;

fn unavailable(why: &str) -> ToolError {
    ToolError::NotAvailable(format!("memory_search_titles 不可用：{why}"))
}

fn spec() -> &'static ToolSpec {
    static SPEC: std::sync::OnceLock<ToolSpec> = std::sync::OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "memory_search_titles".into(),
        description: "Search chapter titles across ALL sessions of the current project's memory \
                      trees (fuzzy top-5). Scoring covers each chapter's title, summary and its \
                      organize-round summary, so vague recollections match too. Use when the \
                      table of contents is long, or the user refers to an earlier conversation \
                      vaguely (\"that performance thing\"). Returns chapter_id + session + \
                      title (+ short summary); call memory_expand with session_id + chapter_id \
                      to read it."
            .into(),
        schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "what to look for in chapter titles" }
            },
            "required": ["query"]
        }),
        side_effect: SideEffect::Pure,
        exposure: Exposure::OnDemand,
    })
}

/// 一条候选（打分用内部结构）。
#[derive(Clone)]
struct Candidate {
    session_id: String,
    session_title: String,
    chapter_id: String,
    title: String,
    summary: String,
    /// 打分文本（v2.142）：标题 + L1 摘要 + 所在轮总结正文。
    index_text: String,
}

pub struct SearchTitlesTool {
    tree_store: TreeStore,
    workspace_of: WorkspaceResolver,
    /// 语义模式出口；None / 未配置 = 关键词降级（组装点传 CooledEmbed）。
    embed: Option<Arc<dyn EmbedPort>>,
    /// 旁车标题读取（hits 带 session_title，模型可引用）。
    sessions_dir: std::path::PathBuf,
}

impl SearchTitlesTool {
    pub fn new(
        tree_store: TreeStore,
        workspace_of: WorkspaceResolver,
        embed: Option<Arc<dyn EmbedPort>>,
        sessions_dir: std::path::PathBuf,
    ) -> Arc<Self> {
        Arc::new(Self {
            tree_store,
            workspace_of,
            embed,
            sessions_dir,
        })
    }

    fn candidates(&self, workspace: &str) -> Vec<Candidate> {
        let session_titles: std::collections::HashMap<String, String> =
            crate::session_log::list(&self.sessions_dir)
                .into_iter()
                .filter_map(|m| m.title.map(|t| (m.id, t)))
                .collect();
        let mut out = Vec::new();
        for (tree, session_id, _) in self.tree_store.scan_workspace(workspace) {
            let session_title = session_titles.get(&session_id).cloned().unwrap_or_default();
            for b in tree.chapters.values().filter(|b| !b.pending) {
                // 所在轮总结正文：章节区间 ⊆ 总结区间即归属（recorder 材料
                // 与总结同轮切片，包含关系成立）；无覆盖 = 空段。
                let round = tree
                    .summaries
                    .iter()
                    .find(|s| b.event_range.0 >= s.from && b.event_range.1 <= s.to)
                    .map(|s| s.text.as_str())
                    .unwrap_or_default();
                out.push(Candidate {
                    session_id: session_id.clone(),
                    session_title: session_title.clone(),
                    chapter_id: b.chapter_id.0.clone(),
                    index_text: format!("{}\n{}\n{}", b.title, b.summary, round),
                    title: b.title.clone(),
                    summary: b.summary.clone(),
                });
            }
        }
        out
    }
}

#[async_trait]
impl Tool for SearchTitlesTool {
    fn spec(&self) -> &ToolSpec {
        spec()
    }

    fn plugin_version(&self) -> &semver::Version {
        &VERSION
    }

    async fn call(&self, ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let session_id = ctx
            .session_id
            .clone()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| unavailable("无会话身份"))?;
        let query = args
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ToolError::Failed {
                tool: "memory_search_titles".into(),
                message: "缺少 query".into(),
            })?;
        let workspace = (self.workspace_of)(&session_id).unwrap_or_default();
        let cands = self.candidates(&workspace);
        if cands.is_empty() {
            let mode = if self.embed.is_some() {
                "semantic"
            } else {
                "keyword"
            };
            return Ok(json!({ "match_mode": mode, "hits": [] }));
        }

        // 语义优先，embedding 注入了但调用失败 = 降级关键词且 match_mode
        // 如实改 "keyword"（不谎报语义命中）
        let (mode, scored) = match &self.embed {
            Some(port) => match self.semantic_hits(port, query, &cands).await {
                Some(hits) => ("semantic", hits),
                None => ("keyword", keyword_hits(query, &cands)),
            },
            None => ("keyword", keyword_hits(query, &cands)),
        };
        let hits: Vec<Value> = scored
            .into_iter()
            .map(|(c, score)| {
                json!({
                    "session_id": c.session_id,
                    "session_title": c.session_title,
                    "chapter_id": c.chapter_id,
                    "title": c.title,
                    "summary": clip_ellipsis(&c.summary, SUMMARY_CLIP),
                    "score": (score * 1000.0).round() / 1000.0,
                })
            })
            .collect();
        Ok(json!({ "match_mode": mode, "hits": hits }))
    }
}

impl SearchTitlesTool {
    /// 语义 top-5。embedding 调用失败 = None（调用方降级关键词 + 如实改
    /// match_mode）。
    async fn semantic_hits(
        &self,
        port: &Arc<dyn EmbedPort>,
        query: &str,
        cands: &[Candidate],
    ) -> Option<Vec<(Candidate, f64)>> {
        let texts: Vec<String> = cands.iter().map(|c| c.index_text.clone()).collect();
        let mut all = vec![query.to_string()];
        all.extend(texts);
        let vecs = port.embed(&all).await.ok()?;
        if vecs.len() != all.len() {
            return None;
        }
        let (qv, title_vecs) = vecs.split_first()?;
        let mut scored: Vec<(Candidate, f64)> = cands
            .iter()
            .cloned()
            .zip(title_vecs.iter())
            .map(|(c, tv)| (c, cosine(qv, tv) as f64))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(TOP_K);
        // 阈值：cosine < 0.2 的命中是噪声，宁缺勿假
        Some(scored.into_iter().filter(|(_, s)| *s >= 0.2).collect())
    }
}

/// 关键词降级：contains 对打分文本（标题+摘要+所在轮总结，大小写不敏感），
/// 命中才返回（无命中 = 空表）。
fn keyword_hits(query: &str, cands: &[Candidate]) -> Vec<(Candidate, f64)> {
    let q = query.to_lowercase();
    cands
        .iter()
        .filter(|c| c.index_text.to_lowercase().contains(&q))
        .cloned()
        .map(|c| (c, 1.0))
        .collect()
}
