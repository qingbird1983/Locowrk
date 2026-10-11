//! ㉘·A·fix 键位身份缓存键 + 工作区上下文行（自 `tests.rs` 批 F-16 拆出，用例本体
//! 逐字未改）：scope_id 变更换键重算 / 零记忆也注入环境块（v2.139）/ 提醒段落在环境块
//! 之后记忆块之前（v2.136）/ 缺省无提醒段 / 上下文行随块整体冻结。

use super::*;

use std::sync::Arc;

// ---- ㉘·A·fix：键位身份缓存键 + 工作区上下文行 ----

/// 键位身份 / 上下文行可变的替身源（模拟归属变更与归属会话）。
struct Scoped {
    calls: std::sync::atomic::AtomicUsize,
    per_call: std::sync::atomic::AtomicUsize,
    scope: std::sync::Mutex<String>,
    line: std::sync::Mutex<Option<String>>,
    /// v2.136 提醒段内文（Some = 有提醒面）。
    repo: std::sync::Mutex<Option<String>>,
}

impl Scoped {
    fn new() -> Self {
        Self {
            calls: std::sync::atomic::AtomicUsize::new(0),
            per_call: std::sync::atomic::AtomicUsize::new(1),
            scope: std::sync::Mutex::new(String::new()),
            line: std::sync::Mutex::new(None),
            repo: std::sync::Mutex::new(None),
        }
    }

    fn count(&self) -> usize {
        self.calls.load(std::sync::atomic::Ordering::SeqCst)
    }
}

impl MemorySource for Scoped {
    fn recall(&self, _session_id: Option<&str>, _limit: usize) -> Vec<MemoryEntry> {
        let n = self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let per = self.per_call.load(std::sync::atomic::Ordering::SeqCst);
        (0..per).map(|i| entry("k", &format!("{n}-{i}"))).collect()
    }

    fn scope_id(&self, _session_id: Option<&str>) -> String {
        self.scope.lock().unwrap().clone()
    }

    fn context_line(&self, _session_id: Option<&str>) -> Option<String> {
        self.line.lock().unwrap().clone()
    }

    fn repo_context(&self, _session_id: Option<&str>) -> Option<String> {
        self.repo.lock().unwrap().clone()
    }
}

/// 会话改归属（scope_id 变化）= 换缓存键 = 首查重算——中途移动项目
/// 不再拿着旧项目的记忆块跑完余生。
#[tokio::test]
async fn scope_change_recomputes_prefetch() {
    let src = Arc::new(Scoped::new());
    let inject = inject(
        src.clone() as std::sync::Arc<dyn MemorySource>,
        cfg_lock(MemoryInjectConfig::default()),
    );
    let first = run_session(&inject, "s1").await.unwrap().system.unwrap();
    assert!(first.contains("- k = 0-0"));
    // 键位没变：复用
    let _ = run_session(&inject, "s1").await.unwrap();
    assert_eq!(src.count(), 1);
    // 归属变更：换键重算，新块来自第二次检索
    *src.scope.lock().unwrap() = "F:\\project-b".into();
    let second = run_session(&inject, "s1").await.unwrap().system.unwrap();
    assert_eq!(src.count(), 2, "scope_id 变更触发重算");
    assert!(second.contains("- k = 1-0"), "重算块进 system");
    // 新键位下继续稳定复用
    let again = run_session(&inject, "s1").await.unwrap().system.unwrap();
    assert_eq!(again, second);
    assert_eq!(src.count(), 2);
}

/// 归属会话零记忆也注入环境块——"看下这个项目"不再靠猜（v2.139 起
/// `<environment>`：项目根 + 日期/平台，日期随预取会话锁死）。
#[tokio::test]
async fn context_line_injected_even_without_memories() {
    let src = Arc::new(Scoped::new());
    src.per_call.store(0, std::sync::atomic::Ordering::SeqCst); // 零记忆
    *src.line.lock().unwrap() = Some("Current project workspace: F:\\AI\\Glance-main".into());
    let inject = inject(
        src as std::sync::Arc<dyn MemorySource>,
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run_session(&inject, "s1")
        .await
        .expect("环境块单独成块")
        .system
        .unwrap();
    assert!(out.contains("<environment>"));
    assert!(out.contains("F:\\AI\\Glance-main"));
    assert!(!out.contains("<persistent_memory>"));
}

/// 提醒段（v2.136 项目侧全提醒制）：归属会话在环境块之后、记忆块之前
/// 注入 `<repo_context>`；无提醒面（None）零出现，组装零变化。
#[tokio::test]
async fn repo_context_injected_after_workspace_line() {
    let src = Arc::new(Scoped::new());
    src.per_call.store(0, std::sync::atomic::Ordering::SeqCst); // 零记忆
    *src.line.lock().unwrap() = Some("Current project workspace: F:\\p".into());
    *src.repo.lock().unwrap() = Some("requirements / handoff / notes reminder".into());
    let inject = inject(
        src as std::sync::Arc<dyn MemorySource>,
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run_session(&inject, "s1").await.unwrap().system.unwrap();
    let env = out.find("</environment>").expect("环境块在");
    let repo = out.find("<repo_context>").expect("提醒段在");
    let mem = out.find("<persistent_memory>").unwrap_or(usize::MAX);
    assert!(env < repo, "提醒段紧随环境块之后");
    assert!(repo < mem, "提醒段先于记忆块（无记忆时 mem 不存在）");
    assert!(out.contains("requirements / handoff / notes reminder"));
}

#[tokio::test]
async fn no_repo_context_by_default() {
    let src = Arc::new(Scoped::new());
    *src.line.lock().unwrap() = Some("Current project workspace: F:\\p".into());
    let inject = inject(
        src as std::sync::Arc<dyn MemorySource>,
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run_session(&inject, "s1").await.unwrap().system.unwrap();
    assert!(!out.contains("<repo_context>"), "缺省实现 = 无提醒段");
}

/// 上下文行随块进预取缓存：跨轮逐字节一致、检索与行查询同拍冻结。
#[tokio::test]
async fn context_line_prefetched_with_block() {
    let src = Arc::new(Scoped::new());
    *src.line.lock().unwrap() = Some("Current project workspace: F:\\p".into());
    let inject = inject(
        src.clone() as std::sync::Arc<dyn MemorySource>,
        cfg_lock(MemoryInjectConfig::default()),
    );
    let a = run_session(&inject, "s1").await.unwrap().system;
    let b = run_session(&inject, "s1").await.unwrap().system;
    assert_eq!(a, b, "上下文行 + 记忆块整体冻结");
}
