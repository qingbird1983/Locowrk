//! 刀③ 判据：触发线两式取小、白名单、错误不清、媒体不清、边界单调、最小节省整批还原、
//! idle 第二通路（specs/09 AC-S2-4 / AC-S2-5）。
//!
//! 一件贯穿全文件的取数纪律：**主层阈值不钉魔法数**，一律从 `estimate_tokens(&msgs)`
//! 推出来（[`over_line`] / [`under_line`]）。钉死一颗数（第一版写的 `Some(100_000)`）
//! 的后果实测过：夹具体量一调整，用例就从"断言被清了"退化成"断言什么都没触发"——
//! 四个用例当时全绿而生产里这一层根本没跑。从当前估算反推 ⇒ 改估算函数或用例体量
//! 都跟着走，`reason` 必须实打实是 `applied` 才算过。

use kernel_agent::estimate_tokens;
use kernel_agent::RequestPayload;
use plugin_api::CallId;
use serde_json::{json, Value};
use session_store::Message;
use std::time::{Duration, SystemTime};

use super::*;

/// 一次往返 = 一条带调用的 assistant + 它的结果。`n` 是组数，`size` 是每条结果的字节数，
/// `from` 是 id 前缀（不同形状的组要能一眼分辨是谁被清了）。
fn rounds(n: usize, size: usize) -> Vec<Message> {
    tagged_rounds("c", n, size)
}

fn tagged_rounds(prefix: &str, n: usize, size: usize) -> Vec<Message> {
    let mut out = Vec::new();
    for i in 0..n {
        let id = CallId(format!("{prefix}{i}"));
        out.push(Message::Assistant {
            content: format!("第{i}轮思考"),
            tool_calls: vec![session_store::ToolCallMsg {
                id: id.clone(),
                tool: "fs_read".into(),
                args: json!({"path": format!("/f{i}.txt")}),
            }],
        });
        out.push(Message::ToolResult {
            call_id: id,
            tool: "fs_read".into(),
            args: json!({}),
            output: json!("x".repeat(size)),
            error: None,
        });
    }
    out
}

/// 主层阈值 = 当前估算值 ⇒ 微压缩线（0.9 那式）必在线下 ⇒ **token 通路确实触发了**。
fn over_line(messages: &[Message]) -> Option<u64> {
    Some(estimate_tokens(messages))
}

/// 主层阈值抬到当前估算的两倍 ⇒ token 通路确实没触发（`below_the_line` 用）。
fn under_line(messages: &[Message]) -> Option<u64> {
    Some(estimate_tokens(messages) * 2)
}

const IDLE_OVER: Option<u64> = Some(61 * 60_000);

fn plan_of(messages: &[Message], main: Option<u64>, idle_ms: Option<u64>) -> MicroPlan {
    plan(&MicroInput {
        messages,
        main_threshold: main,
        idle_ms,
        // 刀⑥：保留组数从常量改成设置项。**夹具给缺省那颗 5**，于是这个文件里
        // 所有断言（"清 c0..c3"那类）一字未改仍然绿 = "缺省 = 改前行为"的实证。
        keep_groups: crate::settings::context::MICRO_KEEP_GROUPS_DEFAULT as usize,
    })
}

/// 触发线 = 两式取小（ZCode `microcompact.ts:79-83` 逐字）。取小拿到的永远是
/// **更早**那条线，两侧的分工要说清：阈值 > 20K 时比例式更低（100K→90K、800K→720K），
/// < 20K 时定值式更低（10K→8K），阈值 ≤ 2000 直接归零。
/// 只测一边查不出"把 min 写成其中一式"那种改法——两边各测一条才算钉住取小。
#[test]
fn threshold_is_the_smaller_of_the_two_forms() {
    assert_eq!(micro_threshold(100_000), 90_000, "100K：0.9 式更低");
    assert_eq!(
        micro_threshold(800_000),
        720_000,
        "800K：还是 0.9 式更低（不是 798K）"
    );
    assert_eq!(
        micro_threshold(10_000),
        8_000,
        "10K：减 2000 那式更低（9_000 不是答案）"
    );
    assert_eq!(micro_threshold(1_000), 0, "极小阈值取到 0 而不是负数");
}

/// 主路径：够旧的结果清成占位，最近 5 组原样留着。
#[test]
fn old_regenerable_results_are_cleared_recent_ones_kept() {
    let msgs = rounds(9, 4_000);
    let p = plan_of(&msgs, over_line(&msgs), None);
    assert_eq!(p.reason, "applied", "{:?}", p.reason);
    assert_eq!(p.cleared.len(), 4, "9 组保留最近 5 ⇒ 清 4 条");
    assert_eq!(p.kept_groups, 5);
    let ids: Vec<String> = p.cleared.iter().map(|c| c.0.clone()).collect();
    assert_eq!(
        ids,
        vec!["c0", "c1", "c2", "c3"],
        "清的最旧的四条，一条不多一条不少"
    );
    assert!(p.to_tokens < p.from_tokens);
}

/// AC-S2-4 的第一半：**边界单调**。清过的不再进候选 ⇒ 第二轮清的是紧随其后的那一批，
/// 已清的那批不会被"回收"也不会重复占名额；同时同一输入重放得到同一输出。
#[test]
fn the_boundary_only_moves_forward() {
    let msgs = rounds(9, 4_000);
    let first = plan_of(&msgs, over_line(&msgs), None);
    assert_eq!(first.reason, "applied");

    // 第一轮应用后的消息列（手工套用 cleared 集）
    let applied: Vec<Message> = apply(&msgs, &first.cleared);

    // 同一份输入重放两次 ⇒ 结果相同（前缀逐字节稳定判据）
    let again = plan_of(&applied, over_line(&applied), None);
    let third = plan_of(&applied, over_line(&applied), None);
    assert_eq!(again.cleared, third.cleared, "重放必须给同一批");

    // 已清的四条不许出现在新的一批里（不回收）
    for old in &first.cleared {
        assert!(
            !again.cleared.contains(old),
            "已清的 {old:?} 被重新计入候选 = 边界回退了"
        );
    }
    // 第二轮名额内只剩 5 组 ⇒ 一组都不该动（边界停在前进过的位置）
    assert_eq!(
        again.reason, "nothing-to-clear",
        "名额内不该动: {:?}",
        again.reason
    );
}

/// 把 cleared 集真套到消息列上（`apply` 与监听器改写路径同形，抽出来给两个用例用）。
fn apply(messages: &[Message], cleared: &[CallId]) -> Vec<Message> {
    messages
        .iter()
        .map(|m| match m {
            Message::ToolResult { call_id, .. } if cleared.contains(call_id) => {
                let mut clone = m.clone();
                if let Message::ToolResult { output, .. } = &mut clone {
                    *output = Value::String(CLEARED.into());
                }
                clone
            }
            other => other.clone(),
        })
        .collect()
}

/// 不到线不清（token 通路）。
#[test]
fn below_the_line_does_nothing() {
    let msgs = rounds(9, 4_000);
    let p = plan_of(&msgs, under_line(&msgs), None);
    assert_eq!(p.reason, "not-triggered");
    assert!(p.cleared.is_empty());
}

/// 实证②：**idle 60 分钟是第二条触发通路**——token 完全没到线，光靠时间也清。
#[test]
fn idle_is_a_second_trigger_path() {
    let msgs = rounds(9, 4_000);
    let just_under = 60 * 60_000;
    let just_over = just_under + 1;

    let at = plan_of(&msgs, under_line(&msgs), Some(just_under));
    assert_eq!(
        at.reason, "not-triggered",
        "恰好 60 分钟不算超（照 ZCode `elapsed > 阈值`）"
    );

    let past = plan_of(&msgs, under_line(&msgs), Some(just_over));
    assert_eq!(past.reason, "applied", "超过 60 分钟就该清");
    assert!(!past.cleared.is_empty());
}

/// 实证①：**错误结果缺省不清**（`clearErrorResults` 缺省关）。
/// 保护网不许指望这一层把失败输出一起抹掉。
#[test]
fn error_results_survive_by_default() {
    let mut msgs = rounds(9, 4_000);
    // 把最旧那条换成"错误结果"：它体量最大，若被清就能一眼看出判据失效
    msgs[1] = Message::ToolResult {
        call_id: CallId("c0".into()),
        tool: "shell".into(),
        args: json!({}),
        output: json!(null),
        error: Some("一整段 stderr 堆栈".repeat(2_000)),
    };
    let p = plan_of(&msgs, over_line(&msgs), None);
    assert_eq!(
        p.reason, "applied",
        "这一轮确实在清（否则本用例什么都没证）"
    );
    assert!(
        !p.cleared.contains(&CallId("c0".into())),
        "失败输出被清了 = 侧门开了"
    );
    // 错误那条不占名额：候选是 c1..c8 共 8 组 ⇒ 清 c1..c3
    let ids: Vec<String> = p.cleared.iter().map(|c| c.0.clone()).collect();
    assert_eq!(ids, vec!["c1", "c2", "c3"], "失败结果占了清位 = 名额算错");
}

/// 白名单：不在名单上的工具结果一律不动（含 MCP 那种动态名与写侧回执）。
#[test]
fn only_whitelisted_tools_are_candidates() {
    for tool in [
        "mcp_notion_search",
        "fs_write",
        "edit_replace",
        "screenshot",
        "memory_list",
    ] {
        let msgs = vec![
            Message::Assistant {
                content: String::new(),
                tool_calls: vec![session_store::ToolCallMsg {
                    id: CallId("c0".into()),
                    tool: tool.into(),
                    args: json!({}),
                }],
            },
            Message::ToolResult {
                call_id: CallId("c0".into()),
                tool: tool.into(),
                args: json!({}),
                output: json!("y".repeat(200_000)),
                error: None,
            },
        ];
        // 单组 + idle 触发：连候选都不该有（进了候选就是白名单失效）
        let p = plan_of(&msgs, None, IDLE_OVER);
        assert_eq!(
            p.reason, "no-candidates",
            "{tool} 不在白名单，连候选都不该有（进了候选就是判据失效）"
        );
        assert!(p.cleared.is_empty());
    }
}

/// 名单本体也要钉一条：九枚、逐字与 specs/09 AC-S2-5 的映射一致，
/// 且**不含**写侧回执与 `mcp_` 前缀（那是两处最容易"顺手加一个"的漂法）。
#[test]
fn the_whitelist_is_the_nine_named_tools() {
    assert_eq!(
        COMPACTABLE_TOOLS.len(),
        9,
        "名单条数变了 = AC-S2-5 要同步改判"
    );
    for tool in COMPACTABLE_TOOLS {
        assert!(
            !tool.starts_with("mcp_"),
            "{tool}：动态拼的名字不可枚举，不许进名单"
        );
        assert!(
            !(tool.starts_with("fs_write") || tool.starts_with("edit_")),
            "{tool}：写侧回执吃 expected_revision，清了代价比重写高"
        );
    }
}

/// 带媒体的结果不清：截图是"当下视口"的东西，重跑拿不到同一张图。
#[test]
fn media_bearing_results_are_protected() {
    let mut msgs: Vec<Message> = vec![
        Message::Assistant {
            content: String::new(),
            tool_calls: vec![session_store::ToolCallMsg {
                id: CallId("m0".into()),
                tool: "fs_read".into(),
                args: json!({}),
            }],
        },
        Message::ToolResult {
            call_id: CallId("m0".into()),
            tool: "fs_read".into(),
            args: json!({}),
            output: json!({"path": "/x", "asset": {"id": "abc", "mime": "image/png"}}),
            error: None,
        },
    ];
    // 再来八组可清的，把上面这条顶到"够旧"的位置（id 前缀分开，否则按 id 清会误伤）
    msgs.extend(tagged_rounds("r", 8, 4_000));
    let p = plan_of(&msgs, None, IDLE_OVER);
    assert_eq!(p.reason, "applied", "本用例必须真的在清（否则什么都没证）");
    assert!(
        !p.cleared.contains(&CallId("m0".into())),
        "带 asset 的结果被清了（媒体兜底判据失效）"
    );
    assert!(!p.cleared.is_empty(), "其余可再生结果仍应可清");
}

/// 实证③：省不到 256 token 就**整批还原**，不清一半。
#[test]
fn tiny_savings_are_rejected_wholesale() {
    // 每条结果本来就短（已被刀② 那枚预算压过），清掉省的字节 < 256 token
    let msgs = rounds(9, 20);
    let p = plan_of(&msgs, over_line(&msgs), IDLE_OVER);
    assert_eq!(p.reason, "below-min-savings");
    assert!(p.cleared.is_empty(), "不足最小节省必须一条都不清");
    assert_eq!(p.to_tokens, p.from_tokens);
}

/// 窗口未知（主层阈值 None）⇒ token 通路不参与，但 idle 仍可清。
/// 这是 2026-10-19 裁定 A 在机械层的同一条姿态：不拿假定的窗口算线。
#[test]
fn unknown_main_threshold_disables_only_the_token_path() {
    let msgs = rounds(9, 4_000);
    assert_eq!(plan_of(&msgs, None, None).reason, "not-triggered");
    assert_eq!(plan_of(&msgs, None, IDLE_OVER).reason, "applied");
}

/// 孤儿结果（历史里没有对应的 assistant 调用，如旧日志残段）自成一组，不丢。
#[test]
fn orphan_results_still_form_a_group() {
    let mut msgs = rounds(9, 4_000);
    msgs.insert(
        0,
        Message::ToolResult {
            call_id: CallId("orphan".into()),
            tool: "fs_read".into(),
            args: json!({}),
            output: json!("z".repeat(4_000)),
            error: None,
        },
    );
    let p = plan_of(&msgs, None, IDLE_OVER);
    assert_eq!(
        p.cleared.len(),
        5,
        "孤儿进组 ⇒ 候选 10 组 ⇒ 清 5 条（它没进组就只有 9 组 4 条）"
    );
    let ids: Vec<String> = p.cleared.iter().map(|c| c.0.clone()).collect();
    assert_eq!(ids[0], "orphan", "最旧的是孤儿那条，它该第一个被清");
}

/// idle 那张表：冷启走播种、只往新走、按会话隔离。
/// 这三条各管一种真实漂法——播种丢了 ⇒ "隔天回来接着聊"那一格永不触发；
/// 回拨把旧时刻盖回去 ⇒ 每请求都算出巨大 idle，减速层变成常驻清道夫。
#[test]
fn the_last_seen_table_seeds_once_and_only_moves_forward() {
    let mut seen = LastSeen::default();
    // 播种次数用 Rc<Cell> 数：闭包持有一份句柄，测试读另一份（直接 &mut 计数会被
    // 闭包的可变借用拦住，读不到）。
    let count = std::rc::Rc::new(std::cell::Cell::new(0u32));
    let seen_seed = std::rc::Rc::clone(&count);
    let seed = move || {
        seen_seed.set(seen_seed.get() + 1);
        1_000
    };
    // 第一次见 s-1：从磁盘 mtime 播种（1_000），此刻 90_000 ⇒ 差 89_000
    assert_eq!(seen.observe("s-1", 90_000, seed), 89_000);
    assert_eq!(count.get(), 1, "播种只该发生在第一次见面");
    // 第二次见：表里已经是 90_000，**不再播种**（计数仍停在 1）
    assert_eq!(seen.observe("s-1", 95_000, || 7_000), 5_000);
    assert_eq!(count.get(), 1, "第二次还播种 = 播种条件写错了");
    // 时钟回拨：不许把较新的时刻倒退，差值取 0（⇒ 不凭空触发 idle）
    assert_eq!(seen.observe("s-1", 90_000, || 7_000), 0);
    assert_eq!(
        seen.observe("s-1", 200_000, || 7_000),
        105_000,
        "回拨后仍从 95_000 起算"
    );
    // 按会话隔离：另一个会话第一次见面也走播种
    assert_eq!(seen.observe("s-2", 3_000, || 2_000), 1_000);
}

// ---------- 接线面（环本体）：上面那些用例全绿也只证明 `plan` 自己对 ----------
//
// 三条各钉一种"单元测了但生产里不发生"的漂法：
// ① 环没有真的改写**发出去的载荷**（`take_request` 守卫还回来的是改过的包？）；
// ② 界面上那枚开关其实关不掉这一层（`settings.context_micro_enabled()` 读空）；
// ③ 冷启动后 idle 那条通路永远拿不到数（表是空的，日志 mtime 没播种）。
// `MicroCompact::new(` 在生产里只住在 `state/waterfall.rs`（判据 SS18c），这里不重复
// 注册面，只把注册后的**行为**跑到终态。

/// 装配一个环 + 事件总线（主层阈值由夹具给，闭包形状与组装点同形）。
fn bus_with(
    settings: Arc<SettingsStore>,
    sessions_dir: std::path::PathBuf,
    main: Option<u64>,
) -> cordis_core::EventBus {
    let bus = cordis_core::EventBus::new();
    bus.on_waterfall(
        points::REQUEST,
        Arc::new(MicroCompact::new(
            settings,
            sessions_dir,
            Arc::new(move |_| main),
        )),
    );
    bus
}

fn payload_of(messages: Vec<Message>, session: Option<&str>) -> Value {
    serde_json::to_value(RequestPayload {
        messages,
        model: None,
        params: json!({}),
        tools: Vec::new(),
        compacted: None,
        system: None,
        session_id: session.map(|s| s.to_string()),
        tree_directory: None,
    })
    .unwrap()
}

fn req_of(v: Value) -> RequestPayload {
    serde_json::from_value(v).unwrap()
}

/// 载荷里已经被换成占位的那几条（按历史顺序的 call_id）。
fn cleared_ids(req: &RequestPayload) -> Vec<String> {
    let placeholder = Value::String(CLEARED.into());
    req.messages
        .iter()
        .filter_map(|m| match m {
            Message::ToolResult {
                call_id, output, ..
            } if *output == placeholder => Some(call_id.0.clone()),
            _ => None,
        })
        .collect()
}

/// ①：环真的改写发送侧消息列，并把这一笔记进 `compacted` 带回位；
/// 同一份输出再过一遍这一环 ⇒ 一字不动、不再记空账（AC-S2-4 的重放判据取的就是这个形状）。
#[tokio::test]
async fn the_ring_rewrites_the_payload_and_books_the_outcome() {
    let root = crate::testutil::work_dir("micro-ring");
    let settings = Arc::new(SettingsStore::open(&root).unwrap());
    let msgs = rounds(9, 4_000);
    let bus = bus_with(
        Arc::clone(&settings),
        root.join("sessions"),
        Some(estimate_tokens(&msgs)),
    );

    let out = bus
        .waterfall(points::REQUEST, payload_of(msgs.clone(), Some("s-ring")))
        .await;
    assert!(
        matches!(out.decision, WaterfallDecision::Continue),
        "这一环不许短路决策环，实测 {:?}",
        out.decision
    );
    let req = req_of(out.payload);
    assert_eq!(
        cleared_ids(&req),
        vec!["c0", "c1", "c2", "c3"],
        "环没改到发出去的载荷（守卫还包时丢改写）就是这个形状"
    );
    let booked = req
        .compacted
        .clone()
        .expect("动过手就必须写 compacted 带回位");
    assert_eq!(
        booked.strategy, "micro",
        "策略名 = ContextCompacted 的渲染依据"
    );
    assert!(
        booked.from_tokens > booked.to_tokens,
        "记账两个数得有意义：{booked:?}"
    );
    assert_eq!(booked.kept_messages, req.messages.len());

    // 重放的是**同一份消息列再发一次请求**（每次 `agent/request` 的账都是新产生的，
    // 所以这里必须把上一轮的带回位置清空再喂进去——直接序列化 `req` 会把旧账当输入）。
    let again = bus
        .waterfall(
            points::REQUEST,
            payload_of(req.messages.clone(), Some("s-ring")),
        )
        .await;
    let req2 = req_of(again.payload);
    assert!(
        req2.compacted.is_none(),
        "名额内没有可清的了，不许再记一笔账（第二轮就是空账本）"
    );
    assert_eq!(
        serde_json::to_string(&req2.messages).unwrap(),
        serde_json::to_string(&req.messages).unwrap(),
        "同一投影重放必须逐字节相同"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// ②：开关关掉 = 一个字节都不动。缺省必须是"开"（`Default` 那份已在 settings 面钉住），
/// 这里另钉"关得掉"——可配置原则那条（界面上的每个限制都要有开关可关）。
#[tokio::test]
async fn the_switch_off_leaves_the_request_untouched() {
    let root = crate::testutil::work_dir("micro-ring-off");
    std::fs::write(
        root.join("settings.json"),
        json!({"context": {"microEnabled": false}}).to_string(),
    )
    .unwrap();
    let settings = Arc::new(SettingsStore::open(&root).unwrap());
    assert!(
        !settings.context_micro_enabled(),
        "夹具本身要先成立（设置没读到 = 本用例什么都没证）"
    );
    let msgs = rounds(9, 4_000);
    let bus = bus_with(
        Arc::clone(&settings),
        root.join("sessions"),
        Some(estimate_tokens(&msgs)),
    );
    let req = req_of(
        bus.waterfall(points::REQUEST, payload_of(msgs.clone(), Some("s-off")))
            .await
            .payload,
    );
    assert!(cleared_ids(&req).is_empty(), "关了还在清 = 开关是个摆设");
    assert!(req.compacted.is_none(), "关了不许留账");
    assert_eq!(
        serde_json::to_string(&req.messages).unwrap(),
        serde_json::to_string(&msgs).unwrap(),
        "关了就一个字节都不许动（与改前逐字节同形）"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// ③：冷启动的 idle 靠会话日志 mtime 播种。两格一起测才证得出播种的**朝向**：
/// 隔天回来（mtime = 61 分钟前）要能触发，新会话（无日志）必须一动不动。
/// token 通路在这里被刻意抬到够不到 ⇒ 这两格的差别只能由播种那条路解释。
#[tokio::test]
async fn cold_start_seeds_idle_from_the_session_log_mtime() {
    let root = crate::testutil::work_dir("micro-idle-seed");
    let settings = Arc::new(SettingsStore::open(&root).unwrap());
    let sessions = root.join("sessions");
    std::fs::create_dir_all(&sessions).unwrap();
    let msgs = rounds(9, 4_000);
    let main = Some(estimate_tokens(&msgs) + 1_000_000);

    // ① 隔天回来：日志 mtime 拨到 61 分钟前（轮末 append ⇒ mtime 就是最后活动时刻）
    let log = sessions.join("s-age.jsonl");
    let file = std::fs::File::create(&log).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(61 * 60))
        .unwrap();
    drop(file);
    let req = req_of(
        bus_with(Arc::clone(&settings), sessions.clone(), main)
            .waterfall(points::REQUEST, payload_of(msgs.clone(), Some("s-age")))
            .await
            .payload,
    );
    assert!(
        !cleared_ids(&req).is_empty(),
        "61 分钟没活动的会话，冷启后第一次请求必须走 idle 通路（播种丢了就是这个样子）"
    );

    // ② 新会话：没有日志可播种 ⇒ 当作"现在"⇒ idle = 0 ⇒ 一条都不清
    let fresh = req_of(
        bus_with(Arc::clone(&settings), sessions.clone(), main)
            .waterfall(
                points::REQUEST,
                payload_of(msgs.clone(), Some("s-brand-new")),
            )
            .await
            .payload,
    );
    assert!(
        cleared_ids(&fresh).is_empty() && fresh.compacted.is_none(),
        "没有日志时不许凭猜测清历史（宁可不清）"
    );
    std::fs::remove_dir_all(&root).ok();
}
