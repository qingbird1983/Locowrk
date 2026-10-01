use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn spec(name: &str, desc: &str) -> ToolSpec {
    ToolSpec {
        name: name.into(),
        description: desc.into(),
        schema: serde_json::json!({}),
        side_effect: plugin_api::SideEffect::Pure,
        exposure: plugin_api::Exposure::OnDemand,
    }
}

/// 假嵌入器：按关键词给二维向量（含 "rust" → [1,0]，"http" → [0,1]），
/// 其余文本取 [0,0]。记录调用次数与累计文本数验证缓存（查询每次必嵌，
/// 缓存省的是目录描述）。
struct FakeEmbed {
    calls: AtomicUsize,
    texts_seen: AtomicUsize,
    fail: bool,
}

#[async_trait]
impl EmbedPort for FakeEmbed {
    async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.texts_seen.fetch_add(texts.len(), Ordering::SeqCst);
        if self.fail {
            return Err("embed backend down".into());
        }
        Ok(texts
            .iter()
            .map(|t| {
                let lower = t.to_lowercase();
                if lower.contains("rust") {
                    vec![1.0, 0.0]
                } else if lower.contains("http") {
                    vec![0.0, 1.0]
                } else {
                    vec![0.0, 0.0]
                }
            })
            .collect())
    }
}

#[tokio::test]
async fn cosine_basics() {
    assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
    assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
    // 维度不齐 / 零向量 = 0，不 panic
    assert_eq!(cosine(&[1.0], &[1.0, 0.0]), 0.0);
    assert_eq!(cosine(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
}

#[tokio::test]
async fn scores_rank_by_similarity_and_cache() {
    let embed = Arc::new(FakeEmbed {
        calls: AtomicUsize::new(0),
        texts_seen: AtomicUsize::new(0),
        fail: false,
    });
    let r = Retriever::new(embed.clone() as Arc<dyn EmbedPort>);
    let catalog = vec![
        spec("git_commit", "commit changes to the rust repository"),
        spec("http_post", "send an http request"),
        spec("fs_read", "read a file"),
    ];
    let scores = r.scores("rust 工具", &catalog).await;
    // rust 命中 git_commit 的描述；fs_read 零向量得 0 分不入表
    assert_eq!(scores.get("git_commit").copied(), Some(1.0));
    assert!(!scores.contains_key("http_post"));
    assert!(!scores.contains_key("fs_read"));
    // 第二次查询：查询本身必嵌（1 文本），目录描述走缓存不再嵌——
    // 累计文本 = 首次 1 查询 + 3 描述 + 第二次 1 查询 = 5
    let again = r.scores("rust", &catalog).await;
    assert_eq!(again.get("git_commit").copied(), Some(1.0));
    assert_eq!(embed.calls.load(Ordering::SeqCst), 2);
    assert_eq!(embed.texts_seen.load(Ordering::SeqCst), 5);
    // 描述变了 → 键失效 → 该描述重新嵌；未变的 http_post/fs_read 仍命中
    let changed = vec![
        spec("git_commit", "totally different rust thing"),
        spec("http_post", "send an http request"),
    ];
    r.scores("rust", &changed).await;
    assert_eq!(embed.calls.load(Ordering::SeqCst), 3);
    assert_eq!(embed.texts_seen.load(Ordering::SeqCst), 7);
}

#[tokio::test]
async fn embed_failure_degrades_to_empty() {
    let r = Retriever::new(Arc::new(FakeEmbed {
        calls: AtomicUsize::new(0),
        texts_seen: AtomicUsize::new(0),
        fail: true,
    }));
    let catalog = vec![spec("a", "rust stuff")];
    assert!(r.scores("rust", &catalog).await.is_empty());
    // 空查询 / 空目录也不打 embedding
    let ok = Arc::new(FakeEmbed {
        calls: AtomicUsize::new(0),
        texts_seen: AtomicUsize::new(0),
        fail: false,
    });
    let r2 = Retriever::new(ok.clone() as Arc<dyn EmbedPort>);
    assert!(r2.scores("", &catalog).await.is_empty());
    assert!(r2.scores("rust", &[]).await.is_empty());
    assert_eq!(ok.calls.load(Ordering::SeqCst), 0);
}
