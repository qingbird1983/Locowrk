//! M1 会话级预取（KV v2 纪律 2）（自 `tests.rs` 批 F-16 拆出，用例本体逐字未改）：
//! 同会话跨轮逐字节冻结且只检索一次 / 换会话换预取 / top_k 变更显式失效重算 /
//! 首查即空也入缓存（指引块同拍冻结）。

use super::*;

use std::sync::Arc;

// ---- M1 会话级预取（KV v2 纪律 2）----

/// 计数检索源：每次调用 value = 调用序号（内容逐次漂移），`per_call` 条。
/// 预取断言的核心道具——预取生效 = 只查一次、块文本跨轮逐字节一致。
struct Counting {
    calls: std::sync::atomic::AtomicUsize,
    per_call: usize,
}

impl Counting {
    fn new(per_call: usize) -> Self {
        Self {
            calls: std::sync::atomic::AtomicUsize::new(0),
            per_call,
        }
    }

    fn count(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl MemorySource for Counting {
    fn recall(&self, _session_id: Option<&str>, _limit: usize) -> Vec<MemoryEntry> {
        let n = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        (0..self.per_call)
            .map(|i| entry("drift", &format!("{n}-{i}")))
            .collect()
    }
}

/// 预取冻结：同一会话连续三轮，system 逐字节一致、检索只发生一次——
/// 记忆库中途写入（逐次漂移的替身）不再改写本会话前缀。
#[tokio::test]
async fn prefetch_freezes_block_across_rounds() {
    let src = Arc::new(Counting::new(1));
    let inject = inject(
        src.clone() as std::sync::Arc<dyn MemorySource>,
        cfg_lock(MemoryInjectConfig::default()),
    );
    let mut systems = Vec::new();
    for _ in 0..3 {
        let out = run_session(&inject, "s1").await.expect("round injects");
        systems.push(out.system);
    }
    assert!(systems[0].is_some());
    assert_eq!(systems[0], systems[1], "相邻轮 system 前缀必须逐字节一致");
    assert_eq!(systems[1], systems[2]);
    assert_eq!(src.count(), 1, "预取生效：检索只在会话首次注入执行一次");
}

/// 换会话 = 换预取：不同会话各自检索一次（互不串块）。
#[tokio::test]
async fn prefetch_is_per_session() {
    let src = Arc::new(Counting::new(1));
    let inject = inject(
        src.clone() as std::sync::Arc<dyn MemorySource>,
        cfg_lock(MemoryInjectConfig::default()),
    );
    let s1 = run_session(&inject, "s1").await.unwrap().system.unwrap();
    let s2 = run_session(&inject, "s2").await.unwrap().system.unwrap();
    assert_ne!(s1, s2, "不同会话各自首查");
    assert_eq!(src.count(), 2);
    // 各自第二轮继续复用各自的块
    let s1_again = run_session(&inject, "s1").await.unwrap().system.unwrap();
    assert_eq!(s1, s1_again);
    assert_eq!(src.count(), 2);
}

/// top_k 变更 = 显式配置变更：重算接受失效；top_k 不变继续复用。
#[tokio::test]
async fn top_k_change_recomputes_prefetch() {
    let src = Arc::new(Counting::new(1));
    let cfg = cfg_lock(MemoryInjectConfig {
        enabled: true,
        top_k: 5,
        ..MemoryInjectConfig::default()
    });
    let inject = inject(src.clone() as std::sync::Arc<dyn MemorySource>, cfg.clone());
    let _ = run_session(&inject, "s1").await.unwrap();
    assert_eq!(src.count(), 1);
    // top_k 没变：继续复用
    let _ = run_session(&inject, "s1").await.unwrap();
    assert_eq!(src.count(), 1);
    // top_k 变了：重算——新块内容来自第二次检索（value = "1-0"）
    cfg.write().unwrap().top_k = 3;
    let out = run_session(&inject, "s1").await.unwrap();
    assert_eq!(src.count(), 2, "top_k 变更触发重算");
    assert!(
        out.system.unwrap().contains("- drift = 1-0"),
        "重算结果进 system"
    );
}

/// 首查即无记忆也缓存（指引块入缓存）：空记忆会话不做每轮空查，
/// 且指引块跨轮逐字节一致（F3：指引随块同吃预取）。
#[tokio::test]
async fn empty_recall_prefetched_too() {
    let src = Arc::new(Counting::new(0));
    let inject = inject(
        src.clone() as std::sync::Arc<dyn MemorySource>,
        cfg_lock(MemoryInjectConfig::default()),
    );
    let first = run_session(&inject, "s1")
        .await
        .expect("指引块随中间件启用注入")
        .system;
    for _ in 0..2 {
        let out = run_session(&inject, "s1")
            .await
            .expect("指引块跨轮复用")
            .system;
        assert_eq!(first, out, "指引块与记忆块同拍冻结");
    }
    assert_eq!(src.count(), 1, "空结果也只查一次");
}
