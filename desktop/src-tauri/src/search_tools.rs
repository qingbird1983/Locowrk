//! search_tools 元工具（exposure 文档 §4.3 / §6.2 / §13）：`always` +
//! `Pure` 的主动拉取兜底——前两层没覆盖的工具，LLM 用它按需检索。
//!
//! 宿主侧内置工具（与 SendEmailTool 同理）：检索索引读"全目录视图"
//! （含 hidden——元工具就是 §4.1 的显式检索通路；停用插件不在 active
//! 实例里，天然不在索引），空结果回退 top-10 高频（§13 常见坑）。向量
//! 检索归 ⑬，本期用关键词匹配（name 命中 2 分 / description 命中 1 分，
//! 大小写不敏感；query 原串也参与匹配以照顾无分词的中文）。
//!
//! 返回 name + description + schema，LLM 下一轮即可调用——resolve 不过滤
//! （⑩②），被截断目录挤出的工具照样可达。

use std::sync::{Arc, OnceLock};

use async_trait::async_trait;
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use serde_json::{Value, json};

use crate::tool_source::KernelToolSource;
use crate::usage::UsageRecorder;

/// 关键词命中上限：结果也是要进上下文的，不能把目录原样倒回去。
const KEYWORD_MAX: usize = 8;
/// 空结果回退的高频条数（§13"兜底返回 top-10 高频工具"）。
const HOT_FALLBACK: u32 = 10;

fn spec() -> &'static ToolSpec {
    static SPEC: OnceLock<ToolSpec> = OnceLock::new();
    SPEC.get_or_init(|| ToolSpec {
        name: "search_tools".into(),
        description: "Search the available tool catalog by keyword. Call this when you are \
                      unsure whether a tool exists for the task; results include name, \
                      description and schema so you can call them on your next turn."
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

/// 元工具本体：持有全目录视图（不含自身）与使用统计。
pub struct SearchToolsTool {
    catalog: Arc<KernelToolSource>,
    usage: Arc<UsageRecorder>,
}

impl SearchToolsTool {
    pub fn new(catalog: Arc<KernelToolSource>, usage: Arc<UsageRecorder>) -> Arc<Self> {
        Arc::new(Self { catalog, usage })
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
        let matched = keyword_search(&catalog, query, KEYWORD_MAX);
        if !matched.is_empty() {
            return Ok(json!({
                "query": query,
                "source": "keyword_match",
                "results": render(&matched),
            }));
        }
        // 空结果回退 top-10 高频（§13）；统计为空 = 没有可推荐的高频，
        // 如实返回 no_match，不拿任意工具充数
        let hot = hot_fallback(&catalog, &self.usage.hot_tools(HOT_FALLBACK), HOT_FALLBACK as usize);
        if hot.is_empty() {
            return Ok(json!({
                "query": query,
                "source": "no_match",
                "results": [],
            }));
        }
        Ok(json!({
            "query": query,
            "source": "hot_fallback",
            "results": render(&hot),
        }))
    }
}

#[cfg(test)]
mod tests;
