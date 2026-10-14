//! 队列 Q9 内核侧用例：**相关性词表的切分**与**内核→宿主契约的下传**。
//!
//! 分工（与 `desktop/src-tauri/src/memory_learn/tests.rs` 对读）：四信号怎么打分是
//! 宿主的事，那里测；这里只测内核的两件事——① 词表从消息流里怎么切出来（含
//! 对稿「不分词」那条订正的实证：中文必须切得出词，否则 L2 对中文永远为 0）；
//! ② 配置面到契约载荷的下传形状（学习档关掉时**词表与开关一起归零**，宿主因此
//! 只需要看 `weights.enabled` 一个条件，半开状态在构造时就不成立）。

use std::sync::{Arc, Mutex};

use session_store::Message;

use super::super::{LearnWeights, RecallQuery};
use super::{
    Fixed, MemoryEntry, MemoryInjectConfig, MemorySource, cfg_lock, entry, inject, run_session,
};
use crate::waterfall::{RequestPayload, points};
use cordis_core::{WaterfallDecision, WaterfallListener};

/// 一次调用抄下来的三件：`limit`、下传的词表、学习档开关。
/// 起别名是 clippy `type_complexity` 的要求（`-D warnings` 拦得住它），不是为了好看。
type Calls = Vec<(usize, Vec<String>, bool)>;

/// 记录型检索源：把每次 `recall_relevant` 的入参抄下来。交付空表是刻意的——
/// 注入块里因此没有记忆行，但词表与开关的下传形状不受影响（本簇只测下传）。
struct Probe {
    seen: Arc<Mutex<Calls>>,
}

impl MemorySource for Probe {
    fn recall(&self, _session_id: Option<&str>, _limit: usize) -> Vec<MemoryEntry> {
        Vec::new()
    }

    fn recall_relevant(
        &self,
        _session_id: Option<&str>,
        limit: usize,
        query: &RecallQuery,
    ) -> Vec<MemoryEntry> {
        self.seen.lock().expect("probe poisoned").push((
            limit,
            query.terms.clone(),
            query.weights.enabled,
        ));
        Vec::new()
    }
}

fn payload_with_users(texts: &[&str]) -> serde_json::Value {
    serde_json::to_value(RequestPayload {
        messages: texts.iter().map(|t| Message::user(*t)).collect(),
        model: None,
        params: serde_json::Value::Object(serde_json::Map::new()),
        tools: Vec::new(),
        compacted: None,
        system: None,
        session_id: Some("s1".into()),
        tree_directory: None,
    })
    .unwrap()
}

/// 同一会话连跑两轮后返回探针看到的调用序列。
///
/// ⚠ "跑两轮"本身就是判据的一半：**第二轮必须命中会话预取缓存**。这条测的不是
/// 词表而是 M1 纪律在 Q9 之后仍然成立——相关性词表是从消息流现切的，如果它
/// 落到缓存之外，逐轮重检索就会改写 system 前缀（历批写死的"缓存头号杀手"）。
async fn probe_run(cfg: MemoryInjectConfig, texts: &[&str]) -> Calls {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let mw = inject(
        Arc::new(Probe {
            seen: Arc::clone(&seen),
        }),
        cfg_lock(cfg),
    );
    let v = payload_with_users(texts);
    for _ in 0..2 {
        match mw.handle(points::REQUEST, &v).await {
            WaterfallDecision::Replace(_) | WaterfallDecision::Continue => {}
            other => panic!("unexpected decision {other:?}"),
        }
    }
    seen.lock().expect("probe poisoned").clone()
}

/// 中文必须切得出词——这是**对稿「不建新索引、不分词」那句的第二条订正**的实证：
/// 中文句子没有空格，纯空白切分会把整句当一个词，`LIKE` 恒不命中 ⇒ relevance
/// 信号对中文永远为 0，等于交付一个永远不响的仪表。2 字滑窗是纯函数、零词典、
/// 零索引，与 `memory_search` 的 LIKE 同档。
#[tokio::test]
async fn cjk_query_produces_terms_and_hits_prefetch_once() {
    let seen = probe_run(MemoryInjectConfig::default(), &["部署用 docker compose"]).await;
    assert_eq!(seen.len(), 1, "会话内只许首查一次，实得 {}", seen.len());
    let (_, terms, enabled) = &seen[0];
    assert!(*enabled);
    assert!(terms.contains(&"部署".to_string()), "实得 {terms:?}");
    // ASCII 侧照旧切整词并小写
    assert!(terms.contains(&"docker".to_string()), "实得 {terms:?}");
    assert!(terms.contains(&"compose".to_string()), "实得 {terms:?}");
}

/// 学习档关掉 = **词表与开关一起归零**：宿主只看 `weights.enabled` 一个条件就够了。
#[tokio::test]
async fn disabling_learn_clears_terms_and_flag_together() {
    let seen = probe_run(
        MemoryInjectConfig {
            learn: LearnWeights {
                enabled: false,
                ..LearnWeights::default()
            },
            ..MemoryInjectConfig::default()
        },
        &["部署用 docker compose"],
    )
    .await;
    assert_eq!(seen.len(), 1);
    let (limit, terms, enabled) = &seen[0];
    assert!(!*enabled);
    assert!(
        terms.is_empty(),
        "关掉学习档还下传词表 = 半开状态，实得 {terms:?}"
    );
    assert_eq!(*limit, MemoryInjectConfig::default().top_k);
}

/// 取数窗口 = 最近 3 条**用户**消息（稿 §3.3 的 N）：数的是用户消息条数而不是
/// 消息总条数——助手长回答与工具结果混进来的话，一条回答能独占整个词表额度。
#[tokio::test]
async fn window_counts_user_messages_only() {
    let texts = ["alphaone", "bravoTwo", "charlie3x", "deltaFour"];
    let seen = probe_run(MemoryInjectConfig::default(), &texts).await;
    let (_, terms, _) = &seen[0];
    // 从尾巴往前数三条 = texts[1..]，最早那条不进窗口
    assert!(terms.contains(&"bravotwo".to_string()), "实得 {terms:?}");
    assert!(terms.contains(&"deltafour".to_string()), "实得 {terms:?}");
    assert!(
        !terms.contains(&"alphaone".to_string()),
        "第四条用户消息不该进窗口，实得 {terms:?}"
    );
}

/// 词表上限与去重：再多对命中计数没有区分度，只增加扫描成本。
///
/// ⚠ 顺带登记一个**实现边界而不是缺陷**：下限只挡长度（<3 字符）与纯数字，
/// **不挡高频虚词**——`the` 这种 3 字母词会进表。挡它需要停用词表，而"无词典"
/// 是本仓与 `memory_search` 同纪律的选择；影响面 = 相关性命中数虚高 1，
/// 而 `k/(k+1)` 只因此抬 0.5 档，不足以翻转排序。要收紧就先谈词典，别在这里
/// 偷偷塞一张硬编码表。
#[tokio::test]
async fn term_list_is_capped_deduplicated_and_noise_filtered() {
    let seen = probe_run(
        MemoryInjectConfig::default(),
        &["docker docker docker compose kubernetes"],
    )
    .await;
    let (_, terms, _) = &seen[0];
    assert_eq!(
        terms.iter().filter(|t| t.as_str() == "docker").count(),
        1,
        "同词重复 = 命中数虚高，实得 {terms:?}"
    );
    assert!(terms.len() <= 8, "词表必须钳制，实得 {}", terms.len());
    let seen = probe_run(MemoryInjectConfig::default(), &["of ab 12345 x"]).await;
    let (_, terms, _) = &seen[0];
    for junk in ["of", "ab", "12345", "x"] {
        assert!(
            !terms.contains(&junk.to_string()),
            "噪声词 {junk} 进了表：{terms:?}"
        );
    }
}

/// **等价性的组装面**：同一份条目集，学习档开与关时 `system` 块**逐字节相同**。
///
/// 这条测的是内核组装（块格式、指引段、环境段都不因本刀改变），配合宿主侧
/// `memory/tests/recall_learn.rs::learn_disabled_returns_current_ordering_and_writes_nothing`
/// ——那条测排序与写回——两半合起来才是稿 §5.4「关掉新机制必须拿到可对照的旧行为」
/// 的完整证明（单独一条都不够：内核半边绿不代表排序没变，宿主半边绿不代表块文本没变）。
#[tokio::test]
async fn learn_off_builds_identical_block_bytes() {
    let src = || Arc::new(Fixed(vec![entry("a", "1"), entry("b", "2")]));
    let on = inject(
        src() as std::sync::Arc<dyn MemorySource>,
        cfg_lock(MemoryInjectConfig::default()),
    );
    let off = inject(
        src() as std::sync::Arc<dyn MemorySource>,
        cfg_lock(MemoryInjectConfig {
            learn: LearnWeights {
                enabled: false,
                ..LearnWeights::default()
            },
            ..MemoryInjectConfig::default()
        }),
    );
    let a = run_session(&on, "s1").await.unwrap().system.unwrap();
    let b = run_session(&off, "s1").await.unwrap().system.unwrap();
    assert_eq!(a, b, "学习档开关动了块字节");
}

/// 块里**不出现任何学习量**（§3.1 原则 4 的可解释性归 L4/Q10 的管理页，不进上下文）：
/// 把 `hit_count` 或分数写进 system 既花 token，又把机器状态泄露给模型，让它
/// 以为自己"知道"这条记忆被用过几次。这条是给将来做 breakdown 的人划的界：
/// **展示面在管理页，不在这个块**。
#[tokio::test]
async fn injected_block_carries_no_learning_numbers() {
    let on = inject(
        Arc::new(Fixed(vec![entry("a", "1"), entry("b", "2")])) as std::sync::Arc<dyn MemorySource>,
        cfg_lock(MemoryInjectConfig::default()),
    );
    let block = run_session(&on, "s1").await.unwrap().system.unwrap();
    for forbidden in ["hit_count", "score", "relevance", "strength", "freshness"] {
        assert!(
            !block.contains(forbidden),
            "学习字段 {forbidden} 进了注入块（它属于管理页，不属于上下文）"
        );
    }
    assert!(block.contains("- a = 1"), "块格式本身不能动：{block}");
}

/// **等价装置**：没覆写 `recall_relevant` 的实现（这里用 `tests::Fixed`）必须
/// 逐字回落到 `recall`——五个既有 fake 与 headless 装配零改动仍然绿，靠的就是
/// 这条缺省支；它同时是「`LearnWeights::default().enabled = false` 是保守缺省
/// 而不是产品缺省」那句注释的机器形式。
#[tokio::test]
async fn default_trait_impl_falls_back_to_recall() {
    let src = Arc::new(Fixed(vec![entry("a", "1"), entry("b", "2")]));
    let q = RecallQuery {
        terms: vec!["Whatever".into()],
        weights: LearnWeights {
            enabled: true,
            ..LearnWeights::default()
        },
    };
    // 忽略 query、走 recall：条数与内容都按 limit 钳制，词表一个字都不参与
    let got = MemorySource::recall_relevant(src.as_ref(), None, 1, &q);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].key, "a");
    // 缺省权重 = 关（未覆写的装配拿到的是现役语义）
    assert!(!LearnWeights::default().enabled);
}
