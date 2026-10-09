//! memory_inject 单测：注入格式 / 配置开关 / 空表降级 / 超长截断 /
//! 追加语义 / 非 REQUEST 点不碰。

use super::{MemoryEntry, MemoryInject, MemoryInjectConfig, MemorySource};
use crate::waterfall::{RequestPayload, points};
use cordis_core::{WaterfallDecision, WaterfallListener};
use serde_json::json;
use std::sync::Arc;

/// 固定返回预设条目的替身检索源。
struct Fixed(Vec<MemoryEntry>);

impl MemorySource for Fixed {
    fn recall(&self, _session_id: Option<&str>, limit: usize) -> Vec<MemoryEntry> {
        self.0.iter().take(limit).cloned().collect()
    }
}

fn entry(key: &str, value: &str) -> MemoryEntry {
    MemoryEntry {
        key: key.into(),
        value: value.into(),
        tags: String::new(),
        updated_at: 0,
        scope: String::new(),
    }
}

fn cfg_lock(c: MemoryInjectConfig) -> std::sync::Arc<std::sync::RwLock<MemoryInjectConfig>> {
    std::sync::Arc::new(std::sync::RwLock::new(c))
}

fn payload_value() -> serde_json::Value {
    serde_json::to_value(RequestPayload {
        messages: Vec::new(),
        model: None,
        params: serde_json::Value::Object(serde_json::Map::new()),
        tools: Vec::new(),
        compacted: None,
        system: None,
        session_id: None,
        tree_directory: None,
    })
    .unwrap()
}

fn inject(
    source: std::sync::Arc<dyn MemorySource>,
    cfg: std::sync::Arc<std::sync::RwLock<MemoryInjectConfig>>,
) -> MemoryInject {
    MemoryInject::new(source, cfg)
}

async fn run(inject: &MemoryInject, key: &str) -> Option<RequestPayload> {
    match inject.handle(key, &payload_value()).await {
        WaterfallDecision::Replace(v) => Some(serde_json::from_value(v).unwrap()),
        WaterfallDecision::Continue => None,
        _ => panic!("unexpected decision"),
    }
}

#[tokio::test]
async fn injects_block_into_system() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("user.lang", "zh-CN")])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    let system = out.system.unwrap();
    assert!(system.contains("<persistent_memory>"));
    assert!(system.contains("- user.lang = zh-CN"));
    assert!(!system.contains("truncated"));
}

#[tokio::test]
async fn top_k_limits_entries() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![
            entry("a", "1"),
            entry("b", "2"),
            entry("c", "3"),
        ])),
        cfg_lock(MemoryInjectConfig {
            top_k: 2,
            ..MemoryInjectConfig::default()
        }),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    let system = out.system.unwrap();
    assert!(system.contains("- a = 1"));
    assert!(system.contains("- b = 2"));
    assert!(!system.contains("- c = 3"));
}

#[tokio::test]
async fn disabled_or_zero_top_k_is_noop() {
    for cfg in [
        MemoryInjectConfig {
            enabled: false,
            ..MemoryInjectConfig::default()
        },
        MemoryInjectConfig {
            enabled: true,
            top_k: 0,
        },
    ] {
        let inject = inject(
            std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
            cfg_lock(cfg),
        );
        assert!(run(&inject, points::REQUEST).await.is_none());
    }
}

/// 空记忆会话（F3 起）：指引块单独注入——主动记忆的本意就是零记忆的
/// 新会话也要知道"该自己记"，不该等攒出第一条记忆才被告知。
#[tokio::test]
async fn empty_recall_still_gets_guidance() {
    let inject = inject(
        std::sync::Arc::new(Fixed(Vec::new())),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.expect("指引块单独成块");
    let system = out.system.unwrap();
    assert!(system.contains("<memory_guidance>"));
    assert!(system.contains("scope=\"global\""));
    assert!(!system.contains("<persistent_memory>"), "零记忆无事实块");
}

#[tokio::test]
async fn long_values_truncated_with_marker() {
    let long = "z".repeat(600); // 指引块文本含 x（"expected"），计数填充换 z
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("big", &long)])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    let system = out.system.unwrap();
    assert!(system.contains("…(truncated)"));
    // 恰好 500 个 z + 标记，而不是 600 个
    assert_eq!(system.matches('z').count(), 500);
}

#[tokio::test]
async fn tags_rendered_when_present() {
    let mut e = entry("k", "v");
    e.tags = "pref".into();
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![e])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.unwrap();
    assert!(out.system.unwrap().contains(" [tags: pref]"));
}

#[tokio::test]
async fn appends_to_existing_system() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let mut payload = payload_value();
    payload["system"] = json!("base instructions");
    match inject.handle(points::REQUEST, &payload).await {
        WaterfallDecision::Replace(v) => {
            let req: RequestPayload = serde_json::from_value(v).unwrap();
            let system = req.system.unwrap();
            assert!(system.starts_with("base instructions"));
            assert!(system.contains("<persistent_memory>"));
        }
        _ => panic!("expected replace"),
    }
}

#[tokio::test]
async fn non_request_key_is_noop() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    assert!(run(&inject, points::PRE_STEP).await.is_none());
}

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

/// 带会话身份的请求。
async fn run_session(inject: &MemoryInject, session_id: &str) -> Option<RequestPayload> {
    let mut v = payload_value();
    v["session_id"] = serde_json::json!(session_id);
    match inject.handle(points::REQUEST, &v).await {
        WaterfallDecision::Replace(v) => Some(serde_json::from_value(v).unwrap()),
        WaterfallDecision::Continue => None,
        _ => panic!("unexpected decision"),
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

/// 归属会话的记忆块带层标（双层记忆：全局 = 用户偏好 / 项目 = 本项目）。
#[tokio::test]
async fn scoped_entries_render_tier_prefix() {
    let mut g = entry("user.lang", "zh-CN");
    g.scope = "global".into();
    let mut p = entry("arch.note", "uses pnpm");
    p.scope = "project".into();
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![g, p])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    let out = run(&inject, points::REQUEST).await.unwrap().system.unwrap();
    assert!(out.contains("- [global] user.lang = zh-CN"));
    assert!(out.contains("- [project] arch.note = uses pnpm"));
}

/// 主动记忆指引（F3 起，v2.139 §5.2 改写）：随注入块尾——在事实块之后；
/// write 主动 / read 按需（「never as a way to start a conversation」
/// 显式禁令在场）与项目文件自维护的指引语义在场。
#[tokio::test]
async fn guidance_appended_at_block_tail() {
    let inject = inject(
        std::sync::Arc::new(Fixed(vec![entry("a", "1")])),
        cfg_lock(MemoryInjectConfig::default()),
    );
    match inject.handle(points::REQUEST, &payload_value()).await {
        WaterfallDecision::Replace(v) => {
            let out: RequestPayload = serde_json::from_value(v).unwrap();
            let out = out.system.unwrap();
            let mem = out.find("<persistent_memory>").expect("事实块在");
            let guide = out.find("<memory_guidance>").expect("指引块在");
            assert!(mem < guide, "指引在注入块尾");
            assert!(
                out.contains("never as a way to start"),
                "read 侧开场禁令在场"
            );
            assert!(out.contains(".Locowrk/"), "项目文件自维护的指引语义在场");
        }
        _ => panic!("expected replace"),
    }
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
