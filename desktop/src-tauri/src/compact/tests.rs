//! 刀④b 判据：主层压缩的切点选择与提示词装配（specs/09 AC-S2-6 的结构那一半 /
//! AC-S2-7 两套条款不合一）。
//!
//! 取数纪律与刀③ 同一条：**阈值不钉魔法数**，一律从夹具自己的 `estimate_tokens` 推
//! （[`at_line`]），否则"到线就跑"这一档测的是空气。

use kernel_agent::estimate_tokens;
use plugin_api::CallId;
use serde_json::json;
use session_store::{ContentPart, Message};

use super::prompt::{
    build_prompt, format_summary, missing_sections, render_history, shrink_history, KEEP_SECURITY,
    KEEP_VERBATIM, NO_TOOLS, TEMPLATE,
};
use super::*;
// 缺省数值从设置面取（不是测试自己钉一颗）：AC-S2-10 要的"缺省 = 刀④之前的硬编码
// 常量"这件事，只有让用例吃设置面那一颗才测得到。
use crate::settings::context::COMPACT_KEEP_TOKENS_DEFAULT;

/// 一轮 = user + assistant（带一次调用）+ 工具结果。`size` 控制工具输出体量，
/// 让"消息条数"与"token 体量"这两件事能被分别调到某个档位上。
fn rounds(n: usize, size: usize) -> Vec<Message> {
    let mut out = Vec::new();
    for i in 0..n {
        out.push(Message::user(format!("第{i}轮问题")));
        out.push(Message::Assistant {
            content: format!("第{i}轮答复"),
            tool_calls: vec![session_store::ToolCallMsg {
                id: CallId(format!("c{i}")),
                tool: "fs_read".into(),
                args: json!({"path": format!("/src/f{i}.rs")}),
            }],
        });
        out.push(Message::ToolResult {
            call_id: CallId(format!("c{i}")),
            tool: "fs_read".into(),
            args: json!({}),
            output: json!("x".repeat(size)),
            error: None,
        });
    }
    out
}

/// 占用恰好站在线上的读数：阈值 = 全量估算 ⇒ `used >= threshold` 恒成立。
fn at_line(messages: &[Message]) -> Option<u64> {
    Some(estimate_tokens(messages))
}

fn plan_of(
    messages: &[Message],
    threshold: Option<u64>,
    used: u64,
    target_pct: u64,
    keep: u64,
    failures: u32,
) -> Plan {
    plan(&PlanInput {
        messages,
        threshold,
        used,
        target_pct,
        keep_tokens: keep,
        failures,
    })
}

// ---------- 要不要压 ----------

/// 裁定 A 在这一层的同一条姿态：窗口未知 ⇒ 不压（不拿假定的窗口做不可逆的大动作）。
#[test]
fn unknown_window_never_compacts() {
    let msgs = rounds(6, 4_000);
    let p = plan_of(&msgs, None, u64::MAX, 50, COMPACT_KEEP_TOKENS_DEFAULT, 0);
    assert_eq!(p.reason, "unknown-window");
    assert_eq!(p.cut, 0, "没拿到线就不许有切点");
}

/// 到线就跑（裁定 C）：占用没到有效阈值 ⇒ 一律不动。
#[test]
fn below_the_line_does_not_compact_even_when_huge() {
    let msgs = rounds(6, 4_000);
    let used = estimate_tokens(&msgs) - 1;
    let p = plan_of(
        &msgs,
        at_line(&msgs),
        used,
        50,
        COMPACT_KEEP_TOKENS_DEFAULT,
        0,
    );
    assert_eq!(p.reason, "below-line");
    // 刚好等于线 ⇒ 就压（边界是 `>=`，两侧各测一次才算钉住）
    let at = plan_of(
        &msgs,
        at_line(&msgs),
        estimate_tokens(&msgs),
        50,
        COMPACT_KEEP_TOKENS_DEFAULT,
        0,
    );
    assert_eq!(at.reason, "planned", "恰好到线就该动手，实测 {}", at.reason);
}

/// 一轮都凑不齐就压 = 把唯一的上下文换成一份空洞摘要（ZCode
/// `hasEnoughMessagesToCompact` 的两条各测一次：组数、以及"里面要有 assistant"）。
#[test]
fn one_round_is_not_enough_to_compact() {
    let one = rounds(1, 4_000);
    assert_eq!(
        plan_of(
            &one,
            at_line(&one),
            estimate_tokens(&one),
            50,
            COMPACT_KEEP_TOKENS_DEFAULT,
            0
        )
        .reason,
        "too-few-rounds"
    );
    // 三句话但没有 assistant 往返 ⇒ 同样不许压
    let only_users: Vec<Message> = vec![Message::user("a"), Message::user("b"), Message::user("c")];
    let p = plan_of(
        &only_users,
        at_line(&only_users),
        estimate_tokens(&only_users),
        50,
        COMPACT_KEEP_TOKENS_DEFAULT,
        0,
    );
    assert_eq!(p.reason, "too-few-rounds", "没有 assistant 就不许压");
}

/// 熔断：连续失败到第 3 次就不再试（2 次还试 ⇒ 上限不是"每次都熔断"）。
#[test]
fn three_consecutive_failures_fuse_the_layer() {
    let msgs = rounds(6, 4_000);
    let used = estimate_tokens(&msgs);
    for (failures, want) in [
        (MAX_COMPACT_FAILURES - 1, "planned"),
        (MAX_COMPACT_FAILURES, "fused"),
    ] {
        let p = plan_of(
            &msgs,
            at_line(&msgs),
            used,
            50,
            COMPACT_KEEP_TOKENS_DEFAULT,
            failures,
        );
        assert_eq!(p.reason, want, "失败 {failures} 次时应当 {}", want);
    }
}

// ---------- 切在哪 ----------

/// 常规：切点落在**一条 user 的边界**上，保留段里有最新那句问题。
/// "绝不从半轮中间开始发"是内核 `tail_user_start` 的同一条纪律——供应商会拒
/// 一段以 tool_result 开头的消息列。
#[test]
fn the_cut_lands_on_a_user_boundary_and_keeps_the_newest_question() {
    let msgs = rounds(8, 4_000);
    let used = estimate_tokens(&msgs);
    let p = plan_of(
        &msgs,
        at_line(&msgs),
        used,
        50,
        COMPACT_KEEP_TOKENS_DEFAULT,
        0,
    );
    assert_eq!(p.reason, "planned");
    assert!(p.cut > 0 && p.cut < msgs.len());
    assert!(
        matches!(msgs[p.cut], Message::User { .. }),
        "切点必须在 user 边界，实测落在 {:?}",
        msgs[p.cut]
    );
    assert!(
        matches!(msgs.last().unwrap(), Message::ToolResult { .. }),
        "夹具末尾是工具结果：保留段该把它一起留着"
    );
    assert!(
        msgs[p.cut..]
            .iter()
            .any(|m| matches!(m, Message::User { .. })),
        "保留段必须含至少一条用户问题"
    );
    // 压到位的可断言形式：留在窗口里的近期原文 + 摘要自己腾的位子 ≤ 目标位。
    // （摘要文本此刻还不存在，实测的 from/to 由 `deep` 在落账那刻算——判据层不许养
    // 一个没人读的预估数。）
    let kept = estimate_tokens(&msgs[p.cut..]);
    let budget = target_budget(estimate_tokens(&msgs), 50);
    assert!(
        kept + summary_reserve() <= budget,
        "保留段 {kept} + 摘要预留 {} 该不超过目标位 {budget}",
        summary_reserve()
    );
}

/// 近期原文额度大到能装下全部、**并且**目标位不比到线更紧 ⇒ 无可压
/// （`nothing-to-summarize`），而不是把整段历史都塞进摘要请求——那正是"为了压缩先撑爆"。
/// 两侧各测一次：只把目标位收紧（50%）就得切，证明这一档真的是"两个前提都宽"造成的，
/// 不是随手凑绿的。
#[test]
fn a_generous_keep_allowance_leaves_nothing_to_summarize() {
    let msgs = rounds(6, 4_000);
    let used = estimate_tokens(&msgs);
    let huge_keep = used + 100_000;
    // 阈值取"全量 + 摘要要腾的那位子"、目标比 100% ⇒ 目标位恰好装得下全部原文 ⇒
    // 没有头部可摘。（只把阈值取成全量是不行的：100% 目标位仍会为摘要让出预留而
    // 动手，那一档由 `a_hundred_percent_target_still_leaves_room_for_the_summary` 钉。）
    let line = used + summary_reserve();
    let p = plan_of(&msgs, Some(line), line, 100, huge_keep, 0);
    assert_eq!(p.reason, "nothing-to-summarize");
    assert_eq!(p.cut, 0);
    assert_eq!(
        plan_of(&msgs, Some(line), line, 50, huge_keep, 0).reason,
        "planned",
        "目标位一收紧就该动手：否则 50% 这颗设置项在短会话里形同不存在"
    );
}

/// 目标位那一刀：预算小到只装得下最新一轮时，**保留段收缩到那一条 user**，
/// 而不是把切点推到末尾、报一个"什么都压不了"。
/// （这一档是修 bug 时补的对照：写成 `next <= cut` 时它照样绿在"全留"上。）
#[test]
fn a_tight_budget_drops_oldest_kept_messages_instead_of_doing_nothing() {
    let msgs = rounds(8, 4_000);
    let used = estimate_tokens(&msgs);
    // 1% ⇒ 预算极小；keep 额度给足，逼这一刀只能由目标位来切
    let p = plan_of(&msgs, at_line(&msgs), used, 1, used, 0);
    assert_eq!(p.reason, "planned", "预算再小也要压，实测 {}", p.reason);
    assert!(matches!(msgs[p.cut], Message::User { .. }));
    // 再往前一条都不是 user 边界了 ⇒ 这一档确实切到了最新那句问题上
    assert!(
        p.cut + 1 >= msgs.len() || !matches!(msgs[p.cut + 1], Message::User { .. }),
        "还有更靠后的 user 边界没被用上，说明目标位那一刀没生效"
    );
}

/// 同一份输入两次判定给同一个切点（组装侧可重放的前提：撤销/重跑都拿同一份投影）。
#[test]
fn the_plan_is_deterministic_for_the_same_input() {
    let msgs = rounds(9, 3_000);
    let used = estimate_tokens(&msgs);
    let a = plan_of(
        &msgs,
        at_line(&msgs),
        used,
        50,
        COMPACT_KEEP_TOKENS_DEFAULT,
        0,
    );
    let b = plan_of(
        &msgs,
        at_line(&msgs),
        used,
        50,
        COMPACT_KEEP_TOKENS_DEFAULT,
        0,
    );
    assert_eq!(a.cut, b.cut);
}

/// 目标位**先给摘要腾地方**再拿去切。这一条只有"扣了预留"才成立：阈值取到线、
/// 目标比取 100%（= 不要求比现在更小），若目标位就是 `threshold × 100%` 本身，
/// 切点一步都不用动 ⇒ 报 `nothing-to-summarize`；扣掉摘要那七节的位子之后才出现
/// "仍然超目标"这一段 ⇒ 该动手。（变异靶：把 `.saturating_sub(summary_reserve())`
/// 删掉，这条立刻红。）
#[test]
fn a_hundred_percent_target_still_leaves_room_for_the_summary() {
    let msgs = rounds(4, 4_000);
    let used = estimate_tokens(&msgs);
    // keep 额度给到能装下全部，逼"是否动手"完全由目标位那一格决定
    let p = plan_of(&msgs, Some(used), used, 100, used + 100_000, 0);
    assert!(
        summary_reserve() > 0,
        "预留位本身不许是 0，否则这条用例是空的"
    );
    assert_eq!(
        p.reason, "planned",
        "100% 目标位也要为摘要腾地方：实测 {} / cut {}",
        p.reason, p.cut
    );
    assert!(p.cut > 0);
}

/// 目标预算的算法只此一处：50% 是缺省，0 与越界都要兜（0% 会让每一轮都"压不到位"，
/// 而且是静默的）。
#[test]
fn target_budget_clamps_both_ends() {
    assert_eq!(target_budget(100_000, 50), 50_000);
    assert_eq!(target_budget(100_000, 0), 1_000, "0 兜成 1%");
    assert_eq!(target_budget(100_000, 250), 100_000, "越界钳到 100%");
    assert_eq!(target_budget(1, 50), 0, "极小阈值取到 0 不溢出");
}

/// overflow 递减带只按**整行**留较新的一截，两端都不许把内容弄没
/// （留空 = 摘要请求喂空气；劈开一行 = 给模型半句历史上没有的话）。
#[test]
fn shrinking_keeps_the_newest_whole_lines_and_never_empties() {
    let text = "a\nb\nc\nd\ne";
    assert_eq!(shrink_history(text, 1.0), text, "100% 就是原样");
    assert_eq!(
        shrink_history(text, 0.6),
        "c\nd\ne",
        "五行的六成 = 尾部三行"
    );
    assert_eq!(shrink_history(text, 0.2), "e", "五行的两成 = 最后一行");
    assert_eq!(shrink_history(text, 0.01), "e", "再小也要留一行");
    assert_eq!(
        shrink_history(text, 9.0),
        text,
        "越界比例不许放大也不许清空"
    );
    assert_eq!(
        shrink_history(text, 0.3),
        "d\ne",
        "向上取整：宁可多留一整行，也不留半行"
    );
    assert_eq!(shrink_history("", 0.5), "", "空输入不炸");
}

// ---------- 提示词装配与验收 ----------

/// 四份稿子都在场，七节节名与 [`SECTIONS`] 同源（模板漂了、验收还按旧的七个节名查
/// = 摘要永远"缺节"，而没人知道为什么）。
#[test]
fn the_prompt_carries_the_seven_sections_from_one_source() {
    let p = build_prompt(None);
    for name in SECTIONS {
        assert!(
            p.contains(&format!("## {name}")) || p.contains(&format!("##{name}")),
            "模板里缺这一节：{name}"
        );
    }
    for name in SUBSECTIONS {
        assert!(p.contains(&format!("### {name}")), "工作状态缺子节：{name}");
    }
    assert!(p.contains("只用文本回答"), "不许调工具那条要在场");
}

/// AC-S2-7 的结构化写法：两套逐字条款**各在自己的段落里**，各自的关键词也在自己的
/// 段里。把任一套并成一句 ⇒ 这一条立刻红（合并后总有一段的关键对象不再在场）。
#[test]
fn the_two_preservation_clauses_stay_two_files() {
    let p = build_prompt(None);
    let security = "# 逐字保留（一）：安全约束与用户原话";
    let verbatim = "# 逐字保留（二）：可查证的符号与串";
    let i_sec = p.find(security).expect("条款（一）不在场");
    let i_ver = p.find(verbatim).expect("条款（二）不在场");
    assert!(i_sec < i_ver, "两条款的顺序也是契约：先重话再准话");
    // 各自的内容段（到下一个 # 标题为止）
    let sec_body = &p[i_sec..i_ver];
    let ver_body = &p[i_ver..];
    for kw in ["安全约束", "逐字原文", "照抄"] {
        assert!(sec_body.contains(kw), "条款（一）丢了「{kw}」= 被并句了");
    }
    for kw in ["文件路径", "错误串", "URL"] {
        assert!(ver_body.contains(kw), "条款（二）丢了「{kw}」= 被并句了");
    }
    // 反面：两套对象不许互相混进对方的段里（混进去就是合成了一句）
    assert!(
        !sec_body.contains("URL"),
        "「URL」跑进了条款（一）= 两件事被写成了一件事"
    );
    assert!(
        !ver_body.contains("安全约束"),
        "「安全约束」跑进条款（二）= 两件事被写成了一件事"
    );
}

/// 自定义压缩指令：给了就插在两份条款之后，空串/全空白 = 与"没给"逐字节相同
/// （可配置原则那条：缺省必须等于不启用时的形状）。
#[test]
fn custom_instructions_are_appended_only_when_meaningful() {
    let bare = build_prompt(None);
    assert_eq!(bare, build_prompt(Some("")), "空串不许改变字节");
    assert_eq!(bare, build_prompt(Some("   \n ")), "全空白不许改变字节");
    let with = build_prompt(Some("只保留与数据库有关的决定"));
    // 骨架四段的顺序与字节必须逐字不动（分隔符在这里再写一遍是断言所需：要证的
    // 正是"拼接本身没被自定义指令挤开"）。
    assert!(
        with.starts_with(&format!(
            "{NO_TOOLS}\n\n{TEMPLATE}\n\n{KEEP_SECURITY}\n{KEEP_VERBATIM}"
        )),
        "附加指令只许追加，不许改前面的骨架"
    );
    assert!(with.contains("只保留与数据库有关的决定"));
    // 尾部那一次"不许调工具"必须仍在最末：附加段不许把它挤到中间去。
    assert!(
        with.trim_end().ends_with(NO_TOOLS.trim_end()),
        "结尾那次提醒丢了 = 摘要请求开始调工具"
    );
}

/// 头尾各钉一次"不许调工具"（ZCode preamble/trailer 的形状），且用的是同一份稿子
/// ——数出现次数 == 2 才证"没复制第二份文本"。
#[test]
fn the_no_tools_rule_is_said_twice_from_one_asset() {
    let p = build_prompt(None);
    assert_eq!(
        p.matches("只用文本回答").count(),
        2,
        "开头一次 + 结尾一次，多一份就是复制了文本"
    );
}

/// 摘要验收的三档：齐 = 通过；缺一节 = 点名；节里空着 = 点名；写「（无）」= 通过。
#[test]
fn validation_names_exactly_the_missing_sections() {
    let good = "## 目标\n做成 X\n\n## 约束与要求\n（无）\n\n## 已定决策\n用 A\n\n\
                ## 工作状态\n### 已完成\n写测试\n### 进行中\n（无）\n### 受阻\n（无）\n\n\
                ## 下一步\n提交\n\n## 相关文件\nsrc/a.rs\n\n## 重要上下文\n实测 648";
    assert!(
        missing_sections(good).is_empty(),
        "齐的摘要不该点名：{:?}",
        missing_sections(good)
    );

    let no_target = good.replace("## 目标\n做成 X\n\n", "\n");
    assert_eq!(missing_sections(&no_target), vec!["目标"]);

    let empty_section = good.replace("## 下一步\n提交", "## 下一步\n   ");
    assert_eq!(missing_sections(&empty_section), vec!["下一步"]);

    let no_state = good.replace("### 受阻\n（无）\n\n", "\n");
    assert_eq!(missing_sections(&no_state), vec!["受阻"]);
}

/// 喂模型的文本要把**调用意图**、失败、附件名字都写出来，并对超长工具输出封顶
/// （不封顶就是"为了压缩先撑爆摘要请求"）。
#[test]
fn rendered_history_keeps_intent_and_caps_bulk() {
    let mut msgs = rounds(2, TOOL_OUTPUT_MAX_CHARS + 500);
    msgs.push(Message::User {
        content: "看下这张图".into(),
        parts: vec![ContentPart::Image {
            asset: session_store::AssetRef {
                id: "a1".into(),
                mime: "image/png".into(),
                name: "截图.png".into(),
            },
        }],
    });
    msgs.push(Message::ToolResult {
        call_id: CallId("c9".into()),
        tool: "shell".into(),
        args: json!({}),
        output: json!(null),
        error: Some("command not found: qmake".into()),
    });
    let text = render_history(&msgs);
    assert!(text.contains("调用 fs_read"), "调用意图要在场：{text}");
    assert!(text.contains("/src/f0.rs"), "参数里的路径要在场");
    assert!(text.contains("已截断"), "超长工具输出必须封顶");
    assert!(
        text.contains("截图.png（image/png）"),
        "附件名字与 mime 要在场"
    );
    assert!(
        text.contains("[工具 shell 失败] command not found"),
        "失败要标出来"
    );
    // 封顶是真的封住了：整段里任何一行都不该比上限长很多
    let longest = text.lines().map(|l| l.chars().count()).max().unwrap_or(0);
    assert!(
        longest <= TOOL_OUTPUT_MAX_CHARS + 80,
        "单行封顶失效，实测最长 {longest} 字符"
    );
}

/// 归一化只动空白与围栏，一个内容字符都不改（改内容 = 拿摘要再去压摘要）。
#[test]
fn formatting_touches_only_wrapping() {
    let raw = "\n```markdown\n## 目标\n做成 X\n\n\n\n## 下一步\n提交\n```\n\n";
    let out = format_summary(raw);
    assert!(!out.contains("```"), "整段围栏该被剥掉：{out}");
    assert!(!out.contains("\n\n\n"), "连续空行该收敛成一段");
    // 内容与标点一字不差：剔掉空白、剔掉**纯围栏行**（剥外层围栏正是格式化的本意），
    // 剩下的字符流必须一模一样。
    let keep = |s: &str| {
        s.lines()
            .filter(|l| !l.trim_start().starts_with("```"))
            .flat_map(|l| l.chars().filter(|c| !c.is_whitespace()))
            .collect::<String>()
    };
    assert_eq!(keep(&out), keep(raw), "内容与标点必须一字不差");
    assert!(
        !out.contains("markdown"),
        "语言标记属于围栏，不许留在正文里：{out}"
    );
    // 不带语言标记的围栏同样剥；带空格的"首行"当正文留着（保守，宁多不少）
    assert_eq!(
        format_summary("```\n## 目标\n做成 X\n```"),
        "## 目标\n做成 X"
    );
    // 行内反引号不是围栏：把它一起吃了 = 摘要里再没有可粘贴的命令
    let inline = "## 相关文件\n跑 `cargo test` 验证";
    assert_eq!(format_summary(inline), inline, "行内代码的反引号不许动");
}
