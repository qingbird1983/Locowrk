//! 刀④c 判据：主层压缩的**接线半**（[`super::deep`]）——占用读数、摘要请求、落账
//! 与熔断计数。与 `tests.rs`（纯判据：要不要压、切在哪、提示词装配）分两爿文件，
//! 是因为这一半要有设置、有会话、有假客户端；把它们混进一爿会让 400 行那条建议线
//! 变成一个真问题，而"判据红"与"接线红"混在一起也难归因。
//!
//! 取数纪律同判据侧：**阈值不钉魔法数**，一律从夹具自己的 `estimate_tokens` 推
//! （[`policy_at`]），否则"到线就跑"这一档测的是空气。

use super::deep::{Compactor, Outcome, STRATEGY};
use super::prompt::{build_prompt, missing_sections};
use super::*;
use crate::runner::UsagePolicy;
use crate::settings::SettingsStore;
use crate::testutil::work_dir;
use async_trait::async_trait;
use kernel_agent::estimate_tokens;
use memory_tree::{MemoryManagerClient, MemoryManagerUsage};
use session_store::{Message, Session, SessionEvent};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex as StdMutex};

/// 记下一切、并被脚本牵着走的摘要客户端（`calls` = 每次调用的 system / user / model）。
struct Scripted {
    calls: StdMutex<Vec<(String, String, Option<String>)>>,
    replies: StdMutex<VecDeque<Result<String, String>>>,
}

impl Scripted {
    fn new(replies: Vec<Result<String, String>>) -> Arc<Self> {
        Arc::new(Self {
            calls: StdMutex::new(Vec::new()),
            replies: StdMutex::new(replies.into()),
        })
    }
    fn count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }
    fn last(&self) -> (String, String, Option<String>) {
        self.calls
            .lock()
            .unwrap()
            .last()
            .cloned()
            .expect("至少调用过一次")
    }
    fn all(&self) -> Vec<(String, String, Option<String>)> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl MemoryManagerClient for Scripted {
    async fn complete(
        &self,
        system: &str,
        user: &str,
        model: Option<&str>,
    ) -> Result<(String, MemoryManagerUsage), String> {
        self.calls.lock().unwrap().push((
            system.to_string(),
            user.to_string(),
            model.map(str::to_string),
        ));
        let usage = MemoryManagerUsage {
            input: 100,
            output: 40,
        };
        self.replies
            .lock()
            .unwrap()
            .pop_front()
            .ok_or_else(|| "脚本没有回复了".to_string())?
            .map(|raw| (raw, usage))
    }
}

/// 七节齐全的摘要 = 被测模块给的那一份（`compact::sample_summary`，一个出处）。
/// 这里只留一个短别名，下面的用例读起来顺一点。
fn good_summary() -> String {
    super::sample_summary()
}

fn settings_with(json: serde_json::Value, tag: &str) -> (Arc<SettingsStore>, std::path::PathBuf) {
    let root = work_dir(tag);
    std::fs::write(root.join("settings.json"), json.to_string()).unwrap();
    (Arc::new(SettingsStore::open(&root).unwrap()), root)
}

/// `rounds` 轮问答，每轮的问题都够长（让 token 体量与轮数都能单独调）。
fn session_with(rounds: usize) -> Session {
    let mut s = Session::new();
    for i in 0..rounds {
        s.append(SessionEvent::UserMessage {
            content: format!("第{i}轮问题：") + &"细节".repeat(60),
            parts: Vec::new(),
        });
        s.append(SessionEvent::AssistantMessage {
            content: format!("第{i}轮答复"),
            tool_calls: vec![],
        });
    }
    s
}

/// 占用恰好站在线上的策略读数（窗口给一个够大的已知值 ⇒ 裁定 A 那一档不参与）。
fn policy_at(session: &Session) -> UsagePolicy {
    let used = estimate_tokens(&session.derive_messages());
    UsagePolicy {
        threshold_tokens: used,
        window_tokens: 1_000_000,
        tools_est_tokens: 0,
        // 压缩通路不读这一格（它经 `Compactor::used` 自己取表），夹具给 0 即可。
        refused_tokens: 0,
    }
}

fn compactor(settings: Arc<SettingsStore>, cli: Arc<Scripted>) -> (Compactor, Arc<Scripted>) {
    compactor_with(settings, cli, Arc::new(crate::compact::Overflow::default()))
}

/// 带指定拒收读数表的那一半（刀⑧ 的用例要自己 note 一张表，所以夹具拆开）。
fn compactor_with(
    settings: Arc<SettingsStore>,
    cli: Arc<Scripted>,
    refusals: Arc<crate::compact::Overflow>,
) -> (Compactor, Arc<Scripted>) {
    (
        Compactor::new(
            settings,
            Arc::clone(&cli) as Arc<dyn MemoryManagerClient>,
            // 熔断表由组装点注入（刀⑤a）：测试里每条用例一枚新的，语义与原来
            // "Compactor 自带一张"完全相同——只是构造点换了地方。
            Arc::new(crate::compact::Failures::default()),
            refusals,
        ),
        cli,
    )
}

/// 刀④ 那档占用读数的**改动之前原文**（只把刀⑧ 新加的拒收那一格去掉，其余一字未动）。
/// 本刀有行为变化 ⇒ 按仓里口径（`tests_hotpath.rs` 那三条的同形做法）留一份参考实现
/// 同场对比：同一份"被拒之后"的夹具，旧式读到的还是被拒之前那颗上报数（= ⓜ 说的自锁），
/// 新式读到的顶到线上。没有这一式，新用例红与绿都说明不了"这一格真的变了"。
fn used_old_shape(session: &Session) -> u64 {
    // 生产里那两枚谓词是 `deep.rs` 的私有的；这里按它原式逐字重述（语义一字不动）。
    let is_usage = |e: &SessionEvent| matches!(e, SessionEvent::LlmUsage { .. });
    let is_compacted = |e: &SessionEvent| matches!(e, SessionEvent::ContextCompacted { .. });
    let stale = match (
        session.events().iter().rposition(is_usage),
        session.events().iter().rposition(is_compacted),
    ) {
        (Some(u), Some(c)) => u < c,
        _ => false,
    };
    if stale {
        estimate_tokens(&session.derive_messages())
    } else {
        crate::context_budget::anchor(session).0
    }
}

/// 夹具自证：脚本给的那份摘要必须过验收。它红了就不是被测对象的问题。
#[test]
fn the_scripted_summary_is_itself_valid() {
    assert!(
        missing_sections(&good_summary()).is_empty(),
        "夹具摘要不齐：{:?}",
        missing_sections(&good_summary())
    );
}

/// 开关关掉 = 一行不写、一次模型往返都不发（可配置原则②的那一格）。
#[tokio::test]
async fn the_switch_off_never_reaches_the_model() {
    let (settings, _) = settings_with(
        serde_json::json!({"context": {"compactEnabled": false}}),
        "compact-off",
    );
    let (cp, cli) = compactor(settings, Scripted::new(vec![]));
    let mut s = session_with(6);
    let p = policy_at(&s);
    let len = s.len();
    assert_eq!(cp.run(&mut s, "s-off", &p).await, Outcome::Off);
    assert_eq!(s.len(), len, "关掉那一格就该一行都不写");
    assert_eq!(cli.count(), 0, "关掉那一格就不该花钱");
}

/// 窗口取不到（`window_tokens = 0`）⇒ 裁定 A：不发摘要、不落账。
/// ⚠ 这条必须单独存在：有效阈值在这一档**原样吃滑杆**（`context_budget` 的算式），
/// 所以拿 `threshold_tokens > 0` 当"窗口已知"的证据会写成假绿。
#[tokio::test]
async fn an_unknown_window_never_reaches_the_model() {
    let (settings, _) = settings_with(serde_json::json!({}), "compact-unknown-window");
    let (cp, cli) = compactor(settings, Scripted::new(vec![Ok(good_summary())]));
    let mut s = session_with(6);
    let used = estimate_tokens(&s.derive_messages());
    let p = UsagePolicy {
        threshold_tokens: used,
        window_tokens: 0,
        tools_est_tokens: 0,
        refused_tokens: 0,
    };
    let out = cp.run(&mut s, "s-window", &p).await;
    assert_eq!(out, Outcome::Skipped("unknown-window"), "实测 {out:?}");
    assert_eq!(cli.count(), 0);
    assert!(!s
        .events()
        .iter()
        .any(|e| matches!(e, SessionEvent::ContextCompacted { .. })));
}

/// 压成了：日志只多一条 `ContextCompacted`（strategy 那一档、kept 与实测两个数都
/// 对得上），投影真的变短，而且头一条是**折叠路径同一个构造函数**造出来的摘要消息。
#[tokio::test]
async fn a_passing_summary_folds_into_one_event_with_measured_tokens() {
    let (settings, _) = settings_with(serde_json::json!({}), "compact-pass");
    let (cp, cli) = compactor(settings, Scripted::new(vec![Ok(good_summary())]));
    let mut s = session_with(6);
    let p = policy_at(&s);
    let before = s.derive_messages().len();
    let out = cp.run(&mut s, "s-pass", &p).await;
    let Outcome::Compacted {
        kept_messages,
        from_tokens,
        to_tokens,
    } = out
    else {
        panic!("应当压成，实测 {out:?}");
    };
    assert_eq!(cli.count(), 1, "一次判定一条摘要往返");
    assert!(
        to_tokens < from_tokens,
        "压完要真的更小：{from_tokens} → {to_tokens}"
    );
    let event = s
        .events()
        .iter()
        .rev()
        .find(|e| matches!(e, SessionEvent::ContextCompacted { .. }))
        .expect("日志里必须有一条压缩事件");
    match event {
        SessionEvent::ContextCompacted {
            kept_messages: k,
            strategy,
            from_tokens: f,
            to_tokens: t,
            summary,
            ts,
        } => {
            assert_eq!(*k, kept_messages, "事件里的 kept 必须就是本次保留的条数");
            assert_eq!(strategy, STRATEGY, "strategy 只许一个出处");
            assert_eq!(
                (*f, *t),
                (from_tokens, to_tokens),
                "界面上看到的两个数 = 事件里那两个"
            );
            assert!(*ts > 0, "落账要带时刻");
            assert!(summary.contains("## 目标"), "摘要存全文：{summary}");
        }
        _ => unreachable!(),
    }
    let after = s.derive_messages();
    assert!(
        after.len() < before,
        "投影要真的变短：{before} → {}",
        after.len()
    );
    assert!(
        matches!(&after[0], Message::User { content, .. }
            if content.starts_with(session_store::SUMMARIZED_MARKER)),
        "折叠后的头一条必须是那条共享构造函数造的摘要消息"
    );
}

/// 占用读数的两档（外加"无上报"那档由上面几条覆盖）：**上报新鲜吃上报、上报陈旧
/// 吃压后投影**。这一档不钉住，本轮每个 step 都会再压一次、每次一条模型往返。
#[tokio::test]
async fn a_stale_usage_report_does_not_make_us_compact_twice() {
    let (settings, _) = settings_with(serde_json::json!({}), "compact-stale");
    let (cp, cli) = compactor(
        settings,
        Scripted::new(vec![Ok(good_summary()), Ok(good_summary())]),
    );
    let mut s = session_with(6);
    // 供应商上报一个"远超全量"的数：新鲜时它就是触发数
    s.append(SessionEvent::LlmUsage {
        input: 900_000,
        output: 10,
        cache_read: 0,
        cache_write: 0,
        reasoning: 0,
        total: 900_010,
        duration_ms: 1,
    });
    assert_eq!(
        cp.used(&s, "s-stale"),
        900_000,
        "上报新鲜就该吃上报（与进度条同一把尺）"
    );
    // 阈值取**投影的全量估算**而不是那颗 900_000：目标位 = 阈值 × 50%，若阈值就是
    // 90 万，预算宽到装得下整个窗口 ⇒ 切点一步不动（`nothing-to-summarize`），
    // 这一档要测的"陈旧之后换尺"根本没机会发生。
    let p = policy_at(&s);
    let out = cp.run(&mut s, "s-stale", &p).await;
    assert!(
        matches!(out, Outcome::Compacted { .. }),
        "到线就该压，实测 {out:?}"
    );
    // 压完之后：那条上报已经是"压缩之前"的数了 ⇒ 换吃压后投影
    assert_eq!(
        cp.used(&s, "s-stale"),
        estimate_tokens(&s.derive_messages()),
        "上报陈旧就必须换尺，否则每一步都重压一次"
    );
    assert_eq!(
        cp.run(&mut s, "s-stale", &p).await,
        Outcome::Skipped("below-line"),
        "压后已经在线下 ⇒ 第二次不许再花钱"
    );
    assert_eq!(cli.count(), 1, "两次判定只发一条摘要往返");
}

/// 刀⑧ · ⓜ 那一格：一次请求被供应商按超窗拒掉 ⇒ **它不会有用量上报**，锚点因此停在
/// 被拒之前那颗数，而历史照长在涨。旧形状在同一份夹具上读到的就是那颗偏低的上报数
/// （= 长跑实测到的"41 个失败轮、微压缩与主层各 0 条落账"那个自锁），新形状吃拒收
/// 读数、下一趟当场起步；压成之后那颗读数必须被清掉——留着它 = 一次旧拒收把这层
/// 永久顶在线上方，正好撞回刀④ 那条"每个 step 重压一次"的坑。
#[tokio::test]
async fn a_refused_request_makes_the_next_pass_compact() {
    let (settings, _) = settings_with(serde_json::json!({}), "compact-refusal");
    let refusals = Arc::new(crate::compact::Overflow::default());
    let (cp, cli) = compactor_with(
        settings,
        Scripted::new(vec![Ok(good_summary()), Ok(good_summary())]),
        Arc::clone(&refusals),
    );
    let mut s = session_with(6);
    let window = estimate_tokens(&s.derive_messages());
    // 上报数只到当前窗口的一半：锚点那一路读到的永远"在线下"，这就是 ⓜ 的形状
    s.append(SessionEvent::LlmUsage {
        input: window / 2,
        output: 10,
        cache_read: 0,
        cache_write: 0,
        reasoning: 0,
        total: window / 2 + 10,
        duration_ms: 1,
    });
    let p = UsagePolicy {
        threshold_tokens: window,
        window_tokens: 1_000_000,
        tools_est_tokens: 0,
        refused_tokens: 0,
    };
    // 参考实现同场对比：旧形状在这一格里分子只有那颗偏低的上报数 ⇒ 永远不动手。
    assert_eq!(used_old_shape(&s), window / 2);
    assert!(
        used_old_shape(&s) < p.threshold_tokens,
        "旧形状读到的是被拒之前那颗上报数 = 自锁的那一手（实测 {} < 阈值 {}）",
        used_old_shape(&s),
        p.threshold_tokens
    );
    // 现场：那一次请求确实发出去了、大小就是这个窗口，然后被拒收
    refusals.note("s-refuse", window);
    assert_eq!(
        cp.used(&s, "s-refuse"),
        window,
        "拒收读数要顶到分子上（三档取最大）"
    );
    let out = cp.run(&mut s, "s-refuse", &p).await;
    assert!(
        matches!(out, Outcome::Compacted { .. }),
        "被拒之后下一趟必须起步，实测 {out:?}"
    );
    assert_eq!(
        refusals.of("s-refuse"),
        None,
        "压成之后旧读数要清掉，否则每一步都重压一次"
    );
    assert_eq!(
        cp.run(&mut s, "s-refuse", &p).await,
        Outcome::Skipped("below-line"),
        "窗口已经小了 ⇒ 第二次不该再花钱"
    );
    assert_eq!(cli.count(), 1, "两次判定只发一条摘要往返");
}

/// 连续三次不合格（缺节 / 调用失败 / 整段被围栏包住剥完是空的）就把这一层熔断：
/// 第四次**一次模型往返都不发**，日志里也一条压缩事件都没有。
#[tokio::test]
async fn three_rejected_summaries_fuse_the_layer() {
    let (settings, _) = settings_with(serde_json::json!({}), "compact-fuse");
    let junk = "## 目标\n只有一句，其余六节都没写\n";
    let fenced = "```\n里面什么都没有\n```";
    let (cp, cli) = compactor(
        settings,
        Scripted::new(vec![
            Ok(junk.to_string()),
            Err("网络断了".to_string()),
            Ok(fenced.to_string()),
        ]),
    );
    let want_whys = ["rejected", "summary-call", "rejected"];
    for (i, want) in want_whys.iter().enumerate() {
        let mut s = session_with(6);
        let p = policy_at(&s);
        let out = cp.run(&mut s, "s-fuse", &p).await;
        match out {
            Outcome::Failed {
                why,
                failures,
                attempts,
            } => {
                assert_eq!(why, *want, "第 {} 次的失败缘由", i + 1);
                assert_eq!(failures, (i + 1) as u32, "失败要一次数一格");
                assert_eq!(attempts, 1, "不合格/断网都不是超窗，不许触发递减重试");
            }
            other => panic!("第 {} 次应当 Failed，实测 {other:?}", i + 1),
        }
        assert!(
            !s.events()
                .iter()
                .any(|e| matches!(e, SessionEvent::ContextCompacted { .. })),
            "不合格的东西不许进日志（第 {} 次）",
            i + 1
        );
    }
    assert_eq!(cli.count(), 3);
    let mut s = session_with(6);
    let p = policy_at(&s);
    assert_eq!(
        cp.run(&mut s, "s-fuse", &p).await,
        Outcome::Skipped("fused"),
        "熔断后再发一条往返 = 上限没生效"
    );
    assert_eq!(cli.count(), 3, "熔断那一格必须一次都不花钱");
}

/// 开关关掉挡的是**自动**动手，挡不住人手上那一下（S2 刀⑦ · AC-S2-11）。与
/// [`the_switch_off_never_reaches_the_model`] 同一份设置、同一个会话夹具，只差调
/// `run` 还是 `run_manual`——所以这一条红的时候，只会是这一枚门的差别。
/// 为什么它必须存在：可配置原则②要的是"可关"，不是"不许用"。把开关做成连手动都
/// 挡 = 界面上那一格长成硬禁止，正是本项目一路在反的形状（同设置页那句
/// 「手动 `/压缩` 不受此限」在压缩层的延伸）。
#[tokio::test]
async fn a_manual_run_goes_ahead_even_with_the_switch_off() {
    let (settings, _) = settings_with(
        serde_json::json!({"context": {"compactEnabled": false}}),
        "compact-manual-off",
    );
    let (cp, cli) = compactor(settings, Scripted::new(vec![Ok(good_summary())]));
    let mut s = session_with(6);
    let p = policy_at(&s);
    let out = cp.run_manual(&mut s, "s-manual-off", &p).await;
    assert!(
        matches!(out, Outcome::Compacted { .. }),
        "开关关掉不该挡人主动要的那一次，实测 {out:?}"
    );
    assert_eq!(cli.count(), 1, "压成就是发了一条往返，不多发");
    assert!(
        s.events()
            .iter()
            .any(|e| matches!(e, SessionEvent::ContextCompacted { .. })),
        "手动压成的账与自动同形：日志里就得有那条压缩事件"
    );
}

/// 熔断合上之后，自动那一路一次都不花钱，而**手动那一路照样压成**，并且这次成功
/// 把表清零（下一趟自动又能动）。这条不是重复 `compact/tests.rs` 里 plan 层那两条
/// 对照：那一层只证判据本身，这一层证的是"deep 真把 `manual` 带到了 plan"——把
/// [`Compactor::run_manual`] 里那枚 `true` 改成 `false`，只有这一条会红
/// （判据层那两条自己传参，仍然绿）。
#[tokio::test]
async fn a_manual_run_goes_ahead_even_when_fused() {
    let fuse = crate::compact::MAX_COMPACT_FAILURES;
    let (settings, _) = settings_with(serde_json::json!({}), "compact-manual-fused");
    let mut script = Vec::new();
    for _ in 0..fuse {
        script.push(Err("网络断了".to_string()));
    }
    // 熔断之后：手动那一次 + 表清零后自动那一次，各要一条能过的摘要
    script.push(Ok(good_summary()));
    script.push(Ok(good_summary()));
    let (cp, cli) = compactor(settings, Scripted::new(script));
    for i in 0..fuse {
        let mut s = session_with(6);
        let p = policy_at(&s);
        let out = cp.run(&mut s, "s-manual-fuse", &p).await;
        assert!(
            matches!(out, Outcome::Failed { .. }),
            "第 {} 次应当 Failed，实测 {out:?}",
            i + 1
        );
    }
    let mut s = session_with(6);
    let p = policy_at(&s);
    assert_eq!(
        cp.run(&mut s, "s-manual-fuse", &p).await,
        Outcome::Skipped("fused"),
        "先证表已经合上（否则下面那一步什么也没证）"
    );
    let spent = cli.count();
    assert_eq!(spent, fuse as usize, "熔断那一格自动路必须一次都不花钱");
    let mut s2 = session_with(6);
    let p2 = policy_at(&s2);
    let out = cp.run_manual(&mut s2, "s-manual-fuse", &p2).await;
    assert!(
        matches!(out, Outcome::Compacted { .. }),
        "熔断不许挡人手那一下，实测 {out:?}"
    );
    assert_eq!(cli.count(), spent + 1, "手动那一次才是一条往返");
    let mut s3 = session_with(6);
    let p3 = policy_at(&s3);
    assert!(
        matches!(
            cp.run(&mut s3, "s-manual-fuse", &p3).await,
            Outcome::Compacted { .. }
        ),
        "这次成功要把表清零——手动只是绕过、表还合着 = 自动那一路永远醒不过来"
    );
}

/// 摘要请求带的三样东西：设置里那颗模型名、四段骨架 + 用户的附加指令、
/// 以及**只有被压掉的那半段**历史（最新那句问题必须留在窗口里，不进摘要请求）。
#[tokio::test]
async fn the_summary_request_carries_the_template_and_only_the_older_half() {
    let (settings, _) = settings_with(
        serde_json::json!({"context": {
            "compactModel": "summary-model-x",
            "compactInstructions": "只保留与构建有关的决定"
        }}),
        "compact-payload",
    );
    let (cp, cli) = compactor(settings, Scripted::new(vec![Ok(good_summary())]));
    let mut s = session_with(6);
    let p = policy_at(&s);
    let out = cp.run(&mut s, "s-payload", &p).await;
    assert!(
        matches!(out, Outcome::Compacted { .. }),
        "这次应当压成，实测 {out:?}"
    );
    let (system, user, model) = cli.last();
    assert_eq!(
        model.as_deref(),
        Some("summary-model-x"),
        "设置里那颗模型名要真的送出去"
    );
    // 骨架逐字节 = `build_prompt(instructions)`（同一个出处，测试不重抄提示词）
    assert_eq!(
        system,
        build_prompt(Some("只保留与构建有关的决定")),
        "摘要请求的 system 就是装配出来的那一份，不许中途再改"
    );
    assert!(
        user.contains("第0轮问题"),
        "被压掉的那段要喂给摘要器：{user}"
    );
    assert!(
        !user.contains("第5轮问题"),
        "最新那句问题留在窗口里，不该出现在摘要请求中"
    );
}

/// 供应商报"太长"的真实形状（判别走 `llm::retry` 那张并集表，这条串命中
/// `maximum context`）。⚠ 测试**不**抄我方那句中文 Fatal 文案——"分类结果
/// `to_string()` 之后还能被同一个谓词认出"这件事由
/// `llm/tests/transport.rs` 那条用例钉，这里再抄一遍就是两个出处。
const TOO_LONG: &str = "HTTP 413: This model's maximum context length is 8192 tokens";

/// 摘要模型可以比主模型小得多（`context.compactModel` 换的是**另一档**，而阈值按
/// 主模型的窗口算）⇒ 摘要请求会自己撞线。这一档要缩了重发，而不是放弃压缩。
#[tokio::test]
async fn an_overflow_failure_retries_with_a_shrunk_history() {
    let (settings, _) = settings_with(serde_json::json!({}), "compact-overflow-retry");
    let (cp, cli) = compactor(
        settings,
        Scripted::new(vec![Err(TOO_LONG.to_string()), Ok(good_summary())]),
    );
    let mut s = session_with(6);
    let p = policy_at(&s);
    let out = cp.run(&mut s, "s-ov", &p).await;
    assert!(
        matches!(out, Outcome::Compacted { .. }),
        "缩一档就该压成，实测 {out:?}"
    );
    let calls = cli.all();
    assert_eq!(calls.len(), 2, "一条超窗就该再发一趟");
    let first = &calls[0].1;
    let second = &calls[1].1;
    assert!(
        second.chars().count() < first.chars().count(),
        "第二趟必须更短：{} → {}",
        first.chars().count(),
        second.chars().count()
    );
    // 缩的是**最旧那一头**：较新的一截必须是较旧那一截的整行后缀
    let head: Vec<&str> = first.lines().rev().collect();
    let tail: Vec<&str> = second.lines().rev().collect();
    assert!(
        head.starts_with(&tail[..]),
        "第二趟必须是第一趟的尾部整行，不许劈开一行：\n{second}"
    );
    assert!(
        !second.contains("第0轮问题"),
        "最旧那段要先出列，否则等于什么都没缩"
    );
}

/// 递减带的**边界**：全量一趟 + `SHRINK_STEPS` 每档一趟，用完就如实放弃；
/// 而一次压缩只算**一个**失败计数（算成四次 = 一轮就把这一层锁死，熔断的语义就变了）。
#[tokio::test]
async fn the_whole_shrink_band_costs_exactly_one_failure() {
    let (settings, _) = settings_with(serde_json::json!({}), "compact-shrink-band");
    let replies = vec![
        Err(TOO_LONG.to_string()),
        Err(TOO_LONG.to_string()),
        Err(TOO_LONG.to_string()),
        Err(TOO_LONG.to_string()),
        Ok(good_summary()),
    ];
    let (cp, cli) = compactor(settings, Scripted::new(replies));
    let mut s = session_with(6);
    let p = policy_at(&s);
    let out = cp.run(&mut s, "s-band", &p).await;
    let want_trips = 1 + SHRINK_STEPS.len() as u32;
    match out {
        Outcome::Failed {
            why,
            failures,
            attempts,
        } => {
            assert_eq!(why, "overflow", "放弃的理由要说得准");
            assert_eq!(
                attempts, want_trips,
                "全量一趟 + 递减带每一档一趟，用完就停"
            );
            assert_eq!(failures, 1, "一次压缩只算一个失败");
        }
        other => panic!("递减带用完应当如实 Failed，实测 {other:?}"),
    }
    assert_eq!(
        cli.count(),
        want_trips as usize,
        "多一档少一档都是把这层的成本说错了"
    );
    assert!(
        !s.events()
            .iter()
            .any(|e| matches!(e, SessionEvent::ContextCompacted { .. })),
        "压不成就一行都不许落账"
    );
}
