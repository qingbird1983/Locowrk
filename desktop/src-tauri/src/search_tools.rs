//! search_tools 元工具（exposure 文档 §4.3 / §6.2 / §13）：`always` +
//! `Pure` 的主动拉取兜底——前两层没覆盖的工具，LLM 用它按需检索。
//!
//! 宿主侧内置工具（与 SendEmailTool 同理）：检索索引读"全目录视图"
//! （含 hidden——元工具就是 §4.1 的显式检索通路；停用插件不在 active
//! 实例里，天然不在索引）。⑬ 起语义优先（§4.2 层次 2：embedding 相似
//! 度排序，一次批量请求嵌查询 + 全目录描述，命中 floor 以上取前 8），
//! embedding 未配置或失败降级关键词（name 命中 2 分 / description 命中
//! 1 分，query 原串参与匹配以照顾中文），仍空回退 top-10 高频（§13 坑）。
//!
//! 返回 name + description + schema，LLM 下一轮即可调用——resolve 不过滤
//! （⑩②），被截断目录挤出的工具照样可达。结果另附 `catalog`：结果之外
//! 的全目录工具**名**（发现性补丁 §4.4——名字是廉价 token，schema 才是
//! 大头；一次检索后全目录名字留在转录里全会话有效）。

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use kernel_agent::{cosine, EmbedPort};
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use serde_json::{json, Value};

use crate::tool_source::KernelToolSource;
use crate::usage::UsageRecorder;

/// 关键词/向量命中上限：结果也是要进上下文的，不能把目录原样倒回去。
const KEYWORD_MAX: usize = 8;
/// 向量命中的相似度下限：低于此的相关性只会误导模型选错工具。
const VECTOR_FLOOR: f32 = 0.25;
/// 空结果回退的高频条数（§13"兜底返回 top-10 高频工具"）。
const HOT_FALLBACK: u32 = 10;

fn spec() -> &'static ToolSpec {
    static SPEC: OnceLock<ToolSpec> = OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "search_tools".into(),
        description: "Search the FULL tool catalog (semantic search with keyword fallback). \
                      The tools in your context are only the high-frequency subset; \
                      low-frequency tools (undo/restore, line-range edits, content search, \
                      git, schedules, ...) stay outside it. Call this BEFORE hand-rolling \
                      a workaround such as shell pipelines or repeated file reads. Results \
                      carry name/description/schema and are immediately callable next turn; \
                      the `catalog` field additionally lists every other tool name in the \
                      full catalog, so nothing stays invisible."
            .into(),
        schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "What you want to do, e.g. 'commit changes' or '记住偏好'"
                }
            },
            "required": ["query"]
        }),
        side_effect: SideEffect::Pure,
        exposure: Exposure::Always,
    })
}

/// 工具名按空格切词 + query 原串一起当关键词（照顾中文整句）。
fn keywords(query: &str) -> Vec<String> {
    let lower = query.trim().to_lowercase();
    let mut tokens: Vec<String> = lower.split_whitespace().map(str::to_owned).collect();
    if !lower.is_empty() {
        tokens.push(lower);
    }
    tokens.sort();
    tokens.dedup();
    tokens
}

/// 关键词打分（纯函数）：name 命中 2 分 / description 命中 1 分，取并集。
fn score_of(spec: &ToolSpec, tokens: &[String]) -> usize {
    let name = spec.name.to_lowercase();
    let desc = spec.description.to_lowercase();
    tokens
        .iter()
        .map(|t| {
            if name.contains(t) {
                2
            } else if desc.contains(t) {
                1
            } else {
                0
            }
        })
        .sum()
}

/// 关键词检索（score 降序、同分按名字典序，确定性），截前 `max` 条。
pub(crate) fn keyword_search(catalog: &[ToolSpec], query: &str, max: usize) -> Vec<ToolSpec> {
    let tokens = keywords(query);
    if tokens.is_empty() {
        return Vec::new();
    }
    let mut scored: Vec<(&ToolSpec, usize)> = catalog
        .iter()
        .map(|s| (s, score_of(s, &tokens)))
        .filter(|(_, score)| *score > 0)
        .collect();
    scored.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.name.cmp(&b.0.name)));
    scored
        .into_iter()
        .take(max)
        .map(|(s, _)| s.clone())
        .collect()
}

/// 向量检索（⑬，§4.2）：`desc_vecs[i]` 与 `catalog[i]` 一一对应（调用方
/// 一次批量嵌入得到），与查询向量做余弦，floor 以上取前 `max` 条。
/// 纯函数，单测友好。
pub(crate) fn vector_rank(
    catalog: &[ToolSpec],
    query_vec: &[f32],
    desc_vecs: &[Vec<f32>],
    max: usize,
) -> Vec<ToolSpec> {
    let mut scored: Vec<(&ToolSpec, f32)> = catalog
        .iter()
        .zip(desc_vecs)
        .map(|(s, vec)| (s, cosine(query_vec, vec)))
        .collect();
    scored.retain(|(_, sim)| *sim >= VECTOR_FLOOR);
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored
        .into_iter()
        .take(max)
        .map(|(s, _)| s.clone())
        .collect()
}

/// 空结果回退（§13）：top 高频 ∩ 当前目录（统计残留的已停用工具不入列）。
pub(crate) fn hot_fallback(
    catalog: &[ToolSpec],
    hot: &[(String, u64)],
    max: usize,
) -> Vec<ToolSpec> {
    hot.iter()
        .filter_map(|(name, _)| catalog.iter().find(|s| &s.name == name))
        .take(max)
        .cloned()
        .collect()
}

fn render(specs: &[ToolSpec]) -> Vec<Value> {
    specs
        .iter()
        .map(|s| {
            json!({
                "name": s.name,
                "description": s.description,
                "side_effect": s.side_effect,
                "schema": s.schema,
            })
        })
        .collect()
}

/// 结果之外的全目录名索引（发现性补丁 §4.4）：只给名字不给 schema——
/// 名字是廉价 token，schema 才是大头；保持目录序，模型看到名字即可
/// 按名再次精确检索（resolve 不过滤，看到即可调用）。
fn remaining_names(catalog: &[ToolSpec], rendered: &[ToolSpec]) -> Vec<String> {
    catalog
        .iter()
        .filter(|s| !rendered.iter().any(|r| r.name == s.name))
        .map(|s| s.name.clone())
        .collect()
}

/// 元工具本体：持有全目录视图（不含自身）、使用统计与可选嵌入器。
pub struct SearchToolsTool {
    catalog: Arc<KernelToolSource>,
    usage: Arc<UsageRecorder>,
    embed: Option<Arc<dyn EmbedPort>>,
}

impl SearchToolsTool {
    /// `embed` = ⑬ 语义检索器（embedding 未配置传 None——退关键词）。
    pub fn new(
        catalog: Arc<KernelToolSource>,
        usage: Arc<UsageRecorder>,
        embed: Option<Arc<dyn EmbedPort>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            catalog,
            usage,
            embed,
        })
    }
}

#[async_trait]
impl Tool for SearchToolsTool {
    fn spec(&self) -> &ToolSpec {
        spec()
    }

    fn plugin_version(&self) -> &Version {
        static V: Version = Version::new(1, 0, 0);
        &V
    }

    /// 纯读操作（Pure）：目录检索 + 统计查询，不产生副作用。
    async fn call(&self, _ctx: &CallCtx, args: Value) -> Result<Value, ToolError> {
        let query = args
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if query.is_empty() {
            return Err(ToolError::Failed {
                tool: spec().name.clone(),
                message: "query must be a non-empty description of what you want to do".into(),
            });
        }
        let catalog = self.catalog.list_all();
        // 语义优先（§4.2）：一次批量嵌查询 + 全目录描述；未配置/失败 =
        // 降级关键词（fail-open，§1 可降级）
        if let Some(embed) = &self.embed {
            let mut texts: Vec<String> = vec![query.to_string()];
            texts.extend(catalog.iter().map(|s| s.description.clone()));
            match embed.embed(&texts).await {
                Ok(vecs) if vecs.len() == texts.len() => {
                    let matched = vector_rank(&catalog, &vecs[0], &vecs[1..], KEYWORD_MAX);
                    if !matched.is_empty() {
                        return Ok(json!({
                            "query": query,
                            "source": "vector_match",
                            "results": render(&matched),
                            "catalog": remaining_names(&catalog, &matched),
                        }));
                    }
                }
                Ok(_) | Err(_) => tracing::warn!("search_tools embedding degraded to keyword"),
            }
        }
        let matched = keyword_search(&catalog, query, KEYWORD_MAX);
        if !matched.is_empty() {
            return Ok(json!({
                "query": query,
                "source": "keyword_match",
                "results": render(&matched),
                "catalog": remaining_names(&catalog, &matched),
            }));
        }
        // 空结果回退 top-10 高频（§13）；统计为空 = 没有可推荐的高频，
        // 如实返回 no_match，不拿任意工具充数
        let hot = hot_fallback(
            &catalog,
            &self.usage.hot_tools(HOT_FALLBACK),
            HOT_FALLBACK as usize,
        );
        if hot.is_empty() {
            return Ok(json!({
                "query": query,
                "source": "no_match",
                "results": [],
                // results 空 → 名字索引即全目录（发现性不断：模型仍能看名识工具）
                "catalog": remaining_names(&catalog, &[]),
            }));
        }
        Ok(json!({
            "query": query,
            "source": "hot_fallback",
            "results": render(&hot),
            "catalog": remaining_names(&catalog, &hot),
        }))
    }
}

#[cfg(test)]
mod tests;
