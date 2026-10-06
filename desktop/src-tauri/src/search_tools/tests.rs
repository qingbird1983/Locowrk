//! search_tools 单测：关键词打分排序 / 空结果回退 top 高频 / no_match /
//! 缺 query 报错 / 目录∩统计（已停用工具不入回退）。

use std::sync::Arc;

use async_trait::async_trait;
use plugin_api::{CallCtx, Exposure, SideEffect, Tool, ToolError, ToolSpec};
use semver::Version;
use serde_json::{json, Value};

use super::{hot_fallback, keyword_search, spec, SearchToolsTool, HOT_FALLBACK, KEYWORD_MAX};
use crate::tool_source::KernelToolSource;
use crate::usage::UsageRecorder;
use kernel_agent::EmbedPort;
use plugin_host::{PluginRegistry, RegistryToolSource};

fn tool_spec(name: &str, desc: &str, exposure: Exposure) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: desc.into(),
        schema: json!({"type": "object"}),
        side_effect: SideEffect::Pure,
        exposure,
    }
}

/// 可注入目录的假内置工具（spec 可换，call 恒 ok）。
struct FakeTool(ToolSpec);

#[async_trait]
impl Tool for FakeTool {
    fn spec(&self) -> &ToolSpec {
        &self.0
    }
    fn plugin_version(&self) -> &Version {
        static V: Version = Version::new(0, 1, 0);
        &V
    }
    async fn call(&self, _ctx: &CallCtx, _args: Value) -> Result<Value, ToolError> {
        Ok(json!({}))
    }
}

fn catalog(builtins: Vec<ToolSpec>) -> Arc<KernelToolSource> {
    let mut src = KernelToolSource::new(RegistryToolSource::new(Arc::new(PluginRegistry::new())));
    for b in builtins {
        src = src.with_builtin(Arc::new(FakeTool(b)));
    }
    Arc::new(src)
}

fn recorder() -> Arc<UsageRecorder> {
    UsageRecorder::disabled()
}

#[test]
fn keyword_search_ranks_name_over_description() {
    let catalog = vec![
        tool_spec("read_file", "grep through file content", Exposure::OnDemand),
        tool_spec("grep_files", "find text", Exposure::OnDemand),
    ];
    let hits = keyword_search(&catalog, "grep", KEYWORD_MAX);
    // grep_files 名字命中 2 分 > read_file 描述命中 1 分
    assert_eq!(hits[0].name, "grep_files");
    assert_eq!(hits.len(), 2);
}

#[test]
fn keyword_search_caps_and_handles_cjk_full_query() {
    let catalog = vec![tool_spec("memory_write", "记住一条事实", Exposure::Always)];
    let hits = keyword_search(&catalog, "记住一条事实", KEYWORD_MAX);
    assert_eq!(hits.len(), 1, "query 原串参与匹配（中文无分词）");
    assert_eq!(hits[0].name, "memory_write");
    // 空白 query 无关键词 → 空表
    assert!(keyword_search(&catalog, "   ", KEYWORD_MAX).is_empty());
}

#[test]
fn hot_fallback_intersects_catalog_in_hot_order() {
    let catalog = vec![
        tool_spec("a", "", Exposure::OnDemand),
        tool_spec("b", "", Exposure::OnDemand),
    ];
    let hot = vec![
        ("ghost".into(), 9),
        ("b".into(), 5),
        ("a".into(), 2),
        ("gone".into(), 1),
    ];
    let picked = hot_fallback(&catalog, &hot, 10);
    let names: Vec<&str> = picked.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        ["b", "a"],
        "统计残留的 ghost / gone 不入列，保持热度序"
    );
}

#[tokio::test]
async fn call_keyword_match_returns_schema() {
    let tool = SearchToolsTool::new(
        catalog(vec![tool_spec("fs_read", "read a file", Exposure::Always)]),
        recorder(),
        None,
    );
    let out = tool
        .call(&CallCtx::new(), json!({"query": "read file"}))
        .await
        .expect("keyword match");
    assert_eq!(out["source"], "keyword_match");
    assert_eq!(out["results"][0]["name"], "fs_read");
    assert!(
        out["results"][0]["schema"].is_object(),
        "带 schema 供下轮调用"
    );
}

#[tokio::test]
async fn call_falls_back_to_hot_tools_then_no_match() {
    let usage_db = {
        let dir = std::env::temp_dir().join(format!(
            "locowrk-search-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        crate::usage::UsageDb::open(&dir.join("usage.sqlite3")).unwrap()
    };
    // 直接落库一条 hot 统计（recorder 后台通道未启动，直连 db）
    let mut tools = std::collections::HashMap::new();
    tools.insert(
        "mem_write".to_string(),
        crate::usage::ToolAgg::of("memory-sqlite", 7, 1_000),
    );
    usage_db.flush(&tools, &Default::default()).unwrap();
    let usage = UsageRecorder::direct(usage_db);

    // 目录里没有 mem_write 命中关键词 → 回退 top 高频给出 mem_write
    let tool = SearchToolsTool::new(
        catalog(vec![tool_spec(
            "mem_write",
            "store a fact",
            Exposure::OnDemand,
        )]),
        Arc::clone(&usage),
        None,
    );
    let out = tool
        .call(&CallCtx::new(), json!({"query": "zzz-no-hit"}))
        .await
        .unwrap();
    assert_eq!(out["source"], "hot_fallback");
    assert_eq!(out["results"][0]["name"], "mem_write");

    // 统计为空 → 如实 no_match，不拿任意工具充数
    let tool2 = SearchToolsTool::new(catalog(vec![]), recorder(), None);
    let out2 = tool2
        .call(&CallCtx::new(), json!({"query": "zzz-no-hit"}))
        .await
        .unwrap();
    assert_eq!(out2["source"], "no_match");
    assert_eq!(out2["results"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn call_rejects_missing_or_empty_query() {
    let tool = SearchToolsTool::new(catalog(vec![]), recorder(), None);
    let err = tool.call(&CallCtx::new(), json!({})).await.unwrap_err();
    assert!(matches!(err, ToolError::Failed { .. }));
    let err2 = tool
        .call(&CallCtx::new(), json!({"query": "  "}))
        .await
        .unwrap_err();
    assert!(matches!(err2, ToolError::Failed { .. }));
}

#[test]
fn spec_is_always_and_pure() {
    let s = spec();
    assert!(matches!(s.exposure, Exposure::Always));
    assert!(matches!(s.side_effect, SideEffect::Pure));
    assert_eq!(s.name, "search_tools");
    // 回退条数与文档一致（§13 top-10）
    assert_eq!(HOT_FALLBACK, 10);
    // 发现性补丁（§4.4）：description 自述「可见清单是子集」+ catalog 索引
    assert!(
        s.description.contains("subset"),
        "自述不完整是发现性第一补丁"
    );
    assert!(s.description.contains("catalog"));
}

/// 名字索引纯函数：排除已渲染项、保持目录序。
#[test]
fn remaining_names_excludes_rendered_keeps_catalog_order() {
    let cat = vec![
        tool_spec("fs_read", "read", Exposure::Always),
        tool_spec("fs_grep", "grep", Exposure::Always),
        tool_spec("git_commit", "commit", Exposure::OnDemand),
    ];
    let rendered = vec![tool_spec("fs_grep", "grep", Exposure::Always)];
    assert_eq!(
        super::remaining_names(&cat, &rendered),
        ["fs_read", "git_commit"]
    );
    assert_eq!(super::remaining_names(&cat, &cat), Vec::<String>::new());
}

/// keyword 命中路径：catalog 列出结果之外的其余工具名（只给名字）。
#[tokio::test]
async fn call_attaches_catalog_index_next_to_results() {
    let tool = SearchToolsTool::new(
        catalog(vec![
            tool_spec("fs_read", "read a file", Exposure::Always),
            tool_spec("fs_grep", "search file content", Exposure::Always),
        ]),
        recorder(),
        None,
    );
    let out = tool
        .call(&CallCtx::new(), json!({"query": "read"}))
        .await
        .unwrap();
    assert_eq!(out["source"], "keyword_match");
    assert_eq!(out["results"][0]["name"], "fs_read");
    let names = out["catalog"].as_array().unwrap();
    assert_eq!(names.len(), 1, "只剩未命中的工具名");
    assert_eq!(names[0], "fs_grep");
}

/// no_match 路径：results 空，catalog 即全目录——检索落空也不断发现性。
#[tokio::test]
async fn no_match_still_lists_full_catalog_names() {
    let tool = SearchToolsTool::new(
        catalog(vec![tool_spec(
            "fs_grep",
            "search file content",
            Exposure::OnDemand,
        )]),
        recorder(),
        None,
    );
    let out = tool
        .call(&CallCtx::new(), json!({"query": "zzz-no-hit"}))
        .await
        .unwrap();
    assert_eq!(out["source"], "no_match");
    assert_eq!(out["catalog"][0], "fs_grep");
}

/// 向量打分纯函数：floor 过滤 + 降序截断。
#[test]
fn vector_rank_filters_and_sorts() {
    let catalog = vec![
        tool_spec("git_commit", "commit changes", Exposure::OnDemand),
        tool_spec("http_post", "send request", Exposure::OnDemand),
        tool_spec("fs_read", "read file", Exposure::OnDemand),
    ];
    let desc_vecs = vec![vec![0.9f32], vec![0.4], vec![0.0]];
    let hits = super::vector_rank(&catalog, &[1.0], &desc_vecs, 8);
    assert_eq!(
        hits.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["git_commit", "http_post"]
    );
    // max 截断
    let hits = super::vector_rank(&catalog, &[1.0], &desc_vecs, 1);
    assert_eq!(hits.len(), 1);
}

/// 语义优先：embedding 可用时走 vector_match；关键词路径只在降级时走。
#[tokio::test]
async fn call_prefers_vector_when_embedder_present() {
    struct Kw;
    #[async_trait]
    impl EmbedPort for Kw {
        async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            Ok(texts
                .iter()
                .map(|t| {
                    if t.contains("rust") {
                        vec![1.0f32]
                    } else {
                        vec![0.0]
                    }
                })
                .collect())
        }
    }
    // 查询与描述都无 "rust"（向量 0 分低于 floor）→ 向量空 → 降级
    // 关键词命中 fs_read
    let tool = SearchToolsTool::new(
        catalog(vec![tool_spec(
            "fs_read",
            "read the file",
            Exposure::OnDemand,
        )]),
        recorder(),
        Some(Arc::new(Kw) as Arc<dyn EmbedPort>),
    );
    let out = tool
        .call(&CallCtx::new(), json!({"query": "read file"}))
        .await
        .unwrap();
    assert_eq!(out["source"], "keyword_match");

    // 描述含 rust → 向量命中优先，不再走关键词
    let tool = SearchToolsTool::new(
        catalog(vec![tool_spec(
            "rust_fmt",
            "format the rust code",
            Exposure::OnDemand,
        )]),
        recorder(),
        Some(Arc::new(Kw) as Arc<dyn EmbedPort>),
    );
    let out = tool
        .call(&CallCtx::new(), json!({"query": "make rust nice"}))
        .await
        .unwrap();
    assert_eq!(out["source"], "vector_match");
    assert_eq!(out["results"][0]["name"], "rust_fmt");

    // 嵌入器失败（Err）→ 降级关键词
    struct Boom;
    #[async_trait]
    impl EmbedPort for Boom {
        async fn embed(&self, _texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
            Err("down".into())
        }
    }
    let tool = SearchToolsTool::new(
        catalog(vec![tool_spec(
            "fs_read",
            "read a file",
            Exposure::OnDemand,
        )]),
        recorder(),
        Some(Arc::new(Boom) as Arc<dyn EmbedPort>),
    );
    let out = tool
        .call(&CallCtx::new(), json!({"query": "read file"}))
        .await
        .unwrap();
    assert_eq!(out["source"], "keyword_match");
}
