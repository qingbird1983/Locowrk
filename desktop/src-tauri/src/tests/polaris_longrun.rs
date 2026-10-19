//! **北极星长会话实测**（S2 刀⑦b · AC-S2-9 三条 + ⓔ + 刀③ 遗留的"日志前后对比"）：
//! 模拟重负载会话连跑 [`RUNS`] 轮（每轮带一条大工具输出），取读数并按三条判据当场判。
//! 装置在 [`polaris_rig`](super::polaris_rig)（含覆盖缺口的说明，读它再读本文件）。
//!
//! 三条判据的取数口径（与 spec 逐条对账）：
//! - **①** 每轮真实 input ≤ 有效阈值 ×1.1。"压缩执行轮瞬时除外"按日志判：那一轮里确有
//!   `ContextCompacted` 落账才豁免。这一成的容差是给"锚点比本次请求滞后一轮"留的，
//!   不是给"压缩根本没起步"留的——所以腿 2（开关全关）里超线的轮无处豁免，必然红。
//! - **②** 全程不出现"必须重开会话 / 手动压缩才能继续"：取两个读数——桩的窗口闸门拒收
//!   次数（超窗的**因**）与整轮会话日志里 `llm/retry.rs:53` 那句文案的命中数（超窗的**果**，
//!   真链路上它是供应商 400/413 之后才出现的字面串）。
//! - **③** 后半程每轮 input 均值 ≤ 前半程 ×1.2（每轮取"末次请求"= 那一轮真正发出去的形状）。
//!
//! 四腿分工：腿 1 三层齐着 = 判据本体；腿 2 两枚开关全关 = 证明这副装置能红（否则腿 1
//! 的绿可能只是负载压根没到线）；腿 3 只留主层 = 机械层不抢先时主层单独扛得住，同时给
//! "主层在长会话里真起过作用"一个正面读数（腿 1 里微压缩先动手，主层可能一次都不起步，
//! 那是裁定 C 的正常结果，不许拿它当主层没接线的证据，也不许拿它当压过了的证据）；
//! **腿 4 是刀⑧加的**：把输出预留钉成 512 复现台账 ⓜ 那个"预留小于一轮净增"的现场，
//! 但要证的却是另一件事——撞窗之后这一层必须**自愈**，而不是从某一轮起全灭。

use super::polaris_rig::*;
use super::*;

use crate::runner::run_turn;
use session_store::Session;

/// 一条长跑的读数。判读与取数分开：所有数齐了以后才跑判据，中途不 assert（一边跑一边
/// 断言会把后面的读数丢掉，红了也看不出是从哪一轮开始漂的）。
struct RunReport {
    tag: String,
    threshold: u64,
    window: u64,
    /// 每轮**末次**请求的 input
    per_turn: Vec<u64>,
    /// 每轮所有请求里的最大值（判据① 用它：压缩发生在一轮的中途）
    turn_max: Vec<u64>,
    /// 每轮新增落账条数，按层分列
    turn_micro: Vec<usize>,
    turn_deep: Vec<usize>,
    /// 以失败收束的轮（带桩当时报的错，红的时候要点名是哪一格挡住的）
    failed_turns: Vec<(u32, String)>,
    rejected: usize,
    summary_calls: usize,
    /// 跑完那一刻拒收读数表里**还留着**的数（`Some` = 有一次被拒的现场从没被任何一趟
    /// 压缩消掉，也就是"顶起步"没兑现）
    leftover_refusal: Option<u64>,
    micro_entries: usize,
    deep_entries: usize,
    /// 日志只追加那一半：中点取的前缀字节数与终态同长度前缀是否逐字节相同
    prefix_equal: bool,
    prefix_len: usize,
    overflow_hits: usize,
}

impl RunReport {
    /// 判据① 的那条线。
    fn cap(&self) -> u64 {
        (self.threshold as f64 * 1.1) as u64
    }
    fn ratios(&self) -> (f64, f64, f64) {
        let half = self.per_turn.len() / 2;
        let (head, tail) = (mean(&self.per_turn[..half]), mean(&self.per_turn[half..]));
        (head, tail, tail / head)
    }
}

/// 会话日志当前的全部字节（前缀快照与终态各取一次）。
fn log_bytes(log: &std::path::Path) -> Vec<u8> {
    let path = session_store::session_file(log, "s-polar").expect("会话日志路径可算");
    std::fs::read(&path).expect("日志读得回")
}

async fn long_run(tag: &str, micro_on: bool, deep_on: bool, reserve: u64) -> RunReport {
    let rig = rig(tag, micro_on, deep_on, reserve);
    let log = rig.root.join("sessions");
    let mut session = Session::new();
    let sink = Collector::default();
    let mut r = RunReport {
        tag: tag.to_string(),
        threshold: rig.threshold,
        window: rig.window,
        per_turn: Vec::new(),
        turn_max: Vec::new(),
        turn_micro: Vec::new(),
        turn_deep: Vec::new(),
        failed_turns: Vec::new(),
        rejected: 0,
        summary_calls: 0,
        leftover_refusal: None,
        micro_entries: 0,
        deep_entries: 0,
        prefix_equal: false,
        prefix_len: 0,
        overflow_hits: 0,
    };
    let mut prefix_probe: Option<Vec<u8>> = None;
    for i in 1..=RUNS {
        let req_from = rig.gate.inputs().len();
        let ev_from = session.events().len();
        let text = format!("第{i}轮：把那份笔记读进来再说要点。") + &"要求".repeat(QUESTION_CHARS);
        if let Err(e) = run_turn(
            &rig.kernel,
            &mut session,
            "s-polar",
            text,
            Vec::new(),
            Some(rig.policy()),
            &sink,
            None,
            Some(&log),
            crate::runner::DEFAULT_MAX_STEPS,
            None,
            None,
            None,
            None,
            Some(&rig.compactor),
        )
        .await
        {
            r.failed_turns.push((i, e));
        }
        let reqs = rig.gate.inputs();
        let this_turn = &reqs[req_from.min(reqs.len())..];
        let evs = session.events();
        r.per_turn.push(this_turn.last().copied().unwrap_or(0));
        r.turn_max
            .push(this_turn.iter().max().copied().unwrap_or(0));
        let (micro, deep) = count_kinds(&evs[ev_from.min(evs.len())..]);
        r.turn_micro.push(micro);
        r.turn_deep.push(deep);
        if i == RUNS / 2 {
            prefix_probe = Some(log_bytes(&log));
        }
    }
    // 落账读数从**磁盘**重放取（ⓔ 要证的就是"这笔账真的落到日志那一行"）
    let path = session_store::session_file(&log, "s-polar").expect("会话日志路径可算");
    let raw = std::fs::read_to_string(&path).expect("日志读得回来");
    let (restored, _) = session_store::load_session(&path).expect("日志重放得回来");
    let (mi, di) = count_kinds(restored.events());
    r.micro_entries = mi;
    r.deep_entries = di;
    r.overflow_hits = raw.matches(OVERFLOW_SENTENCE).count();
    r.rejected = rig.gate.rejected();
    r.summary_calls = rig.summarizer.calls();
    r.leftover_refusal = rig.refusals.of("s-polar");
    if let Some(probe) = prefix_probe {
        r.prefix_len = probe.len();
        // `starts_with` 就是"只追加"那句话的形状：终态必须以中点那份前缀开头。
        // 终态连中点那么长都没有 ⇒ 直接判 false（不许在这里 panic 把读数丢掉）。
        r.prefix_equal = log_bytes(&log).starts_with(&probe);
    }
    r
}

/// 把读数落成文件（进程外可读、提交正文引它；工作树不该脏 ⇒ 写系统临时目录）。
fn dump(r: &RunReport) {
    let (head, tail, ratio) = r.ratios();
    let mut s = format!(
        concat!(
            "# 北极星长跑读数 · {}\n\n",
            "窗口 {} / 有效阈值 {} / 判据① 容差（阈值×1.1）{}\n\n",
            "| 轮 | 末次 input | 峰值 input | 本轮微压缩 | 本轮主层 | 失败 |\n|---|---|---|---|---|---|\n"
        ),
        r.tag, r.window, r.threshold, r.cap()
    );
    for i in 0..r.per_turn.len() {
        s.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} |\n",
            i + 1,
            r.per_turn[i],
            r.turn_max[i],
            r.turn_micro[i],
            r.turn_deep[i],
            r.failed_turns
                .iter()
                .find(|(n, _)| *n == i as u32 + 1)
                .map(|(_, e)| e.as_str())
                .unwrap_or("—")
        ));
    }
    s.push_str(&format!(
        concat!(
            "\n拒收（超窗的因）{} 次 · 失败轮 {} 个 · 日志里「{}」命中（超窗的果）{} 次\n",
            "落账累计：微压缩 {} 条 / 主层 {} 条 · 摘要往返 {} 趟 · 跑完仍挂着的拒收读数 {}\n",
            "前半均值 {:.0} · 后半均值 {:.0} ⇒ 比值 {:.3}\n",
            "峰值最大 {} · 中点前缀 {} 字节，终态同长度逐字节相同 = {}\n"
        ),
        r.rejected,
        r.failed_turns.len(),
        OVERFLOW_SENTENCE,
        r.overflow_hits,
        r.micro_entries,
        r.deep_entries,
        r.summary_calls,
        r.leftover_refusal.unwrap_or(0),
        head,
        tail,
        ratio,
        r.turn_max.iter().max().copied().unwrap_or(0),
        r.prefix_len,
        r.prefix_equal
    ));
    let out = std::env::temp_dir().join(format!("{}.md", r.tag));
    std::fs::write(&out, s).ok();
    eprintln!("[北极星长跑 {}] 读数已写 {}", r.tag, out.display());
}

/// 判据①＋②＋③ 一起过（腿 1 与腿 3 共用这三条；差别只在"哪一层必须动过手"）。
fn judge_ac_s2_9(r: &RunReport) {
    assert!(
        r.failed_turns.is_empty(),
        "全程不许有轮以失败收束（首个失败轮 {}：{}）",
        r.failed_turns.first().map(|(n, _)| *n).unwrap_or(0),
        r.failed_turns
            .first()
            .map(|(_, e)| e.as_str())
            .unwrap_or("")
    );
    assert_eq!(
        r.rejected, 0,
        "桩的窗口闸门一次都不该拒收（超窗的因）；实测 {} 次",
        r.rejected
    );
    assert_eq!(
        r.overflow_hits, 0,
        "整轮日志里「{}」的命中必须是 0（超窗的果）",
        OVERFLOW_SENTENCE
    );
    for (i, mx) in r.turn_max.iter().enumerate() {
        let exempt = r.turn_micro[i] + r.turn_deep[i] > 0;
        assert!(
            *mx <= r.cap() || exempt,
            "第 {} 轮峰值 input {} 超过阈值×1.1（{}），而那一轮没有任何压缩落账可豁免",
            i + 1,
            mx,
            r.cap()
        );
    }
    let (head, tail, ratio) = r.ratios();
    assert!(
        ratio <= 1.2,
        "后半程每轮均值 {:.0} 超过前半程 {:.0} 的 1.2 倍（实测比值 {:.3}）",
        tail,
        head,
        ratio
    );
}

/// 腿 1：三层齐着跑 44 轮 —— AC-S2-9 三条 + ⓔ（微压缩那笔账落到会话日志那一行）。
#[tokio::test]
async fn a_hundred_heavy_turns_stay_inside_the_window() {
    let r = long_run("s2-polaris-on", true, true, MAX_OUTPUT_TOKENS).await;
    dump(&r);
    judge_ac_s2_9(&r);
    assert!(
        r.micro_entries > 0,
        "ⓔ 未落：从磁盘重放的日志里一条 `strategy:\"micro\"` 都没有 ⇒ 这跑没测到微压缩那一层"
    );
    assert!(
        r.prefix_equal,
        "会话日志在中点取过的前缀（{} 字节）跑完之后必须逐字节相同——日志只许追加",
        r.prefix_len
    );
}

/// 腿 2（对照）：两枚开关都关掉，同一条脚本该撞穿窗口。
///
/// 这一条只为证明上面那副装置能红：判据本身一个字不改，只改配置。撞不穿说明负载压根
/// 没到预算边界，那腿 1 的绿就没有证明力（那是"什么都没测"而不是"测过且通过"）。
#[tokio::test]
async fn with_the_layers_off_the_same_script_breaks_through() {
    let r = long_run("s2-polaris-off", false, false, MAX_OUTPUT_TOKENS).await;
    dump(&r);
    let broke = r.rejected > 0 || !r.failed_turns.is_empty();
    assert!(
        broke,
        "对照跑没撞穿（拒收 {} 次 / 失败轮 {} 个）⇒ 负载没到线，腿 1 那跑的绿不算数",
        r.rejected,
        r.failed_turns.len()
    );
    assert_eq!(
        r.micro_entries + r.deep_entries,
        0,
        "开关关掉之后还落账 = 那两枚开关不管事（可配置原则②的反面）"
    );
}

/// 腿 3：只留主层（微压缩关）——机械层不抢先时，主层单独也扛得住，且它在这条链上
/// 真起过作用（落账 > 0）。
#[tokio::test]
async fn the_deep_layer_alone_holds_the_window() {
    let r = long_run("s2-polaris-deep-only", false, true, MAX_OUTPUT_TOKENS).await;
    dump(&r);
    judge_ac_s2_9(&r);
    assert!(
        r.deep_entries > 0,
        "主层一次都没落账（摘要往返 {} 趟）⇒ 这条链上深度压缩没起作用，腿 3 不算过",
        r.summary_calls
    );
}

/// 腿 4（S2 刀⑧ · ⓜ 的跨进程证据）：把输出预留钉成 512，**并把微压缩关掉**——机械层
/// 帮不上时，那一格预留才是唯一挡在窗口前面的东西，而这正是登记 ⓜ 时的现场（开着微压缩
/// 时它把占用摁在自己那条线下面，这一腿压根撞不到窗，实测过一次：拒收 0 次 ⇒ 什么也没测）。
/// 要看到三件事：确实撞了窗（`rejected > 0`）、被拒之后主层被顶起步（落账 > 0）、失败轮是
/// **稀疏的**而不是从某轮起全灭。旧形状就是实测记下的那一版：44 轮里 41 个失败轮、微压缩与
/// 主层各 0 条落账——开关全开着，界面上连一句"压缩没起作用"都没有。
#[tokio::test]
async fn a_refused_turn_does_not_lock_the_layer() {
    let r = long_run("s2-polaris-tight-reserve", false, true, 512).await;
    dump(&r);
    assert!(
        r.rejected > 0,
        "这一腿得先真的撞窗（预留 512 远小于一轮净增），一次都没拒就说明配方不对"
    );
    assert!(
        r.deep_entries > 0,
        "撞窗之后主层一条账都没落（摘要往返 {} 趟）= ⓜ 那个自锁还在：被拒的那一次\
         没有用量上报，只吃锚点的触发线永远追不上已经涨过的窗口",
        r.summary_calls
    );
    let last_failed = r.failed_turns.iter().map(|(n, _)| *n).max().unwrap_or(0);
    assert!(
        last_failed < RUNS,
        "最后一个失败轮 {last_failed} 之后再没有跑完的轮 = 从那儿起全灭 = 自锁没解开"
    );
    assert!(
        r.failed_turns.len() * 4 <= RUNS as usize,
        "失败轮 {} 个超过四分之一的轮数（首个 {}）= 被拒之后没立刻把这一层顶起步，\
         后面每轮都在撞窗",
        r.failed_turns.len(),
        r.failed_turns.first().map(|(n, _)| *n).unwrap_or(0)
    );
    assert!(
        r.leftover_refusal.is_none(),
        "跑完那一刻表里还留着读数 {:?} = 有次被拒的现场从没被任何一趟压缩消掉",
        r.leftover_refusal
    );
}
