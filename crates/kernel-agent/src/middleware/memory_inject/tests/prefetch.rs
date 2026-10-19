//! M1 会话级预取（KV v2 纪律 2）：同会话跨轮逐字节冻结且只取数一次 / 换会话换预取 /
//! top_k 变更显式失效重算 / 首查即空也入缓存（指引块同拍冻结）。
//!
//! ⚠ **S3 刀① 换了道具**：这四条原本靠 `recall` 返回"逐次漂移的正文条目"来证预取生效，
//! 而正文注入那一档已经下线 ⇒ 漂移源换成**索引内文**（`global_index` 每次返回调用序号）。
//! 判据本身一个字没改：证的还是"跨轮逐字节一致 + 每会话至多取数一次"。
//! 其中 `top_k_change_recomputes_prefetch` 那条是 `settings.rs::memory_inject_config`
//! 头注点名的**承重件**（"不许把权重塞进缓存失效判据"靠它兜住），所以 top_k 字段
//! 与它进失效判据的形状在本刀**照旧不动**。

use super::*;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// 计数取数源：每次问 `global_index` 就交一条内容漂移的指针行（"漂移"= 预取若失效
/// 就会在 system 里留下痕迹），`per_call` 决定它每次交几行（0 = 首查即空）。
/// 项目段恒 `None`：让 `count()` 就是"本会话真取了几次数"。
struct Drifting {
    calls: AtomicUsize,
    per_call: usize,
}

impl Drifting {
    fn new(per_call: usize) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            per_call,
        }
    }

    fn count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl MemorySource for Drifting {
    fn recall(&self, _session_id: Option<&str>, _limit: usize) -> Vec<MemoryEntry> {
        Vec::new()
    }

    fn global_index(&self, _session_id: Option<&str>) -> Option<String> {
        let n = self.calls.fetch_add(1, Ordering::SeqCst);
        if self.per_call == 0 {
            return None;
        }
        Some(
            (0..self.per_call)
                .map(|i| format!("- [drift-{i}](drift.md) — {n}-{i}"))
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}

/// 预取冻结：同一会话连续三轮，system 逐字节一致、取数只发生一次——
/// 索引中途变化（逐次漂移的替身）不再改写本会话前缀。
#[tokio::test]
async fn prefetch_freezes_block_across_rounds() {
    let src = Arc::new(Drifting::new(1));
    let inject = inject(
        src.clone() as Arc<dyn MemorySource>,
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
    assert_eq!(src.count(), 1, "预取生效：取数只在会话首次注入执行一次");
}

/// 换会话 = 换预取：不同会话各自取数一次（互不串块）。
#[tokio::test]
async fn prefetch_is_per_session() {
    let src = Arc::new(Drifting::new(1));
    let inject = inject(
        src.clone() as Arc<dyn MemorySource>,
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
    let src = Arc::new(Drifting::new(1));
    let cfg = cfg_lock(MemoryInjectConfig {
        enabled: true,
        top_k: 5,
        ..MemoryInjectConfig::default()
    });
    let inject = inject(src.clone() as Arc<dyn MemorySource>, cfg.clone());
    let _ = run_session(&inject, "s1").await.unwrap();
    assert_eq!(src.count(), 1);
    // top_k 没变：继续复用
    let _ = run_session(&inject, "s1").await.unwrap();
    assert_eq!(src.count(), 1);
    // top_k 变了：重算——新块内容来自第二次取数（drift 值 = "1-0"）
    cfg.write().unwrap().top_k = 3;
    let out = run_session(&inject, "s1").await.unwrap();
    assert_eq!(src.count(), 2, "top_k 变更触发重算");
    assert!(out.system.unwrap().contains("— 1-0"), "重算结果进 system");
}

/// 首查即无索引也缓存（指引块入缓存）：空索引会话不做每轮空查，
/// 且指引块跨轮逐字节一致（F3：指引随块同吃预取）。
#[tokio::test]
async fn empty_index_prefetched_too() {
    let src = Arc::new(Drifting::new(0));
    let inject = inject(
        src.clone() as Arc<dyn MemorySource>,
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
        assert_eq!(first, out, "指引块与索引块同拍冻结");
    }
    assert_eq!(src.count(), 1, "空结果也只查一次");
}
