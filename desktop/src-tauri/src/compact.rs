//! 主层深度压缩（S2 刀④ · 方案稿 §B 那一层的主位）：占用到线时，把**较旧的那一段**
//! 交给模型写成一份七节摘要，较新的一段保持原文，合成"摘要 + 近期原文"这一份新的
//! 上下文窗口。会话日志一行不改——压缩是组装期的投影：折叠由 `session-store` 的
//! `ContextCompacted` 承担（`session/project.rs::compact_messages`），撤销 = 回到那条
//! 事件之前（`compaction_rewind_restores_full_history` 钉着）。
//!
//! 本文件只放**纯判据**（要不要压、压到哪一条、哪一段进摘要），提示词装配与
//! 摘要验收在 [`prompt`]，落日志那一半在 [`deep`]。为什么挂在 runner 的 step 边界，
//! 而不是 specs/09 计划行初稿写的 `agent/request` 决策环（2026-10-19 落点订正；三条
//! 理由都是读实 `kernel/request.rs` 之后才成立的，已同步写进那份 spec 的刀④条目）：
//!
//! 1. **重试会把摘要烧成 N 份**。`request_with_retry` 是一个 `loop`，每趟重试都重新
//!    跑一遍 REQUEST 环（`kernel/request.rs:149-157`）。主层一次就是一条模型往返，
//!    重试三次 = 三份摘要三份钱，而后两份永远用不上。
//! 2. **请求失败时这份账会丢**。同一段里 `compacted` 只在 `Ok` 分支带回（`:188-199`），
//!    失败走 `?` 直接返回——挂在环上时，"摘要已经生成"与"日志里有一条
//!    `ContextCompacted`"间断了一节：超限失败那一轮恰恰最需要这份摘要，结果它被丢掉、
//!    下一轮重算一遍。
//! 3. **触发数必须是界面上看到的那把尺**。进度条与整理触发线共用 `context_budget`
//!    的锚点（v2.138「看到的 = 触发用的」），锚点读会话日志里最后一条 `LlmUsage`；
//!    环上的监听器只拿得到载荷（`RequestPayload`），只能吃字符估算 ⇒ 与裁定 C
//!    「机械层压完后主层到线照跑」所依赖的那把尺不同源。
//!
//! 挂在 step 边界三条都顺：`session.append(ContextCompacted)` 当场落日志，内核下一步
//! 自己 `derive_messages` 就是压后的窗口。
//!
//! 参照值逐字来源（2026-10-19 按本地源码读实）：保留近期原文的额度 15_000 token 与
//! 单条工具输出上限 1_250 字符、应急递减带 `[0.7, 0.5, 0.35]` 都取自 OpenCode
//! `session/compaction.ts`（`:197 keep: 15_000` / `:100 TOOL_OUTPUT_MAX_CHARS = 1_250` /
//! `:95 SHRINK_STEPS`）；失败上限 3 取自 ZCode `compact/manual.ts:55`
//! （`MAX_COMPACT_PROMPT_TOO_LONG_RETRIES = 3`）。**只有目标比 50% 是我们自己的裁定**
//! （两份参照里都没有这颗数——OpenCode 用 `keep` 反推窗口余量，ZCode 用阈值减缓冲），
//! 所以它做成设置项 `context.compactTargetPct`，缺省 50 而不是假装照抄谁。

pub(crate) mod deep;
pub(crate) mod prompt;

use std::collections::HashMap;
use std::sync::Mutex;

use kernel_agent::estimate_tokens;
use session_store::Message;

/// 连续失败几次就熔断不再压（ZCode `manual.ts:55`）。
pub(crate) const MAX_COMPACT_FAILURES: u32 = 3;

/// 熔断线：失败计数到这颗数就断（[`MAX_COMPACT_FAILURES`] 的**唯一比较点**）。
///
/// 抽成函数而不是在 [`plan`] 里就地写 `>=`：比较点只有一个，将来调那颗数时
/// 不会漏掉第二处。S4 刀① 退役滑窗保护网后它只剩一个调用方，但仍然不从调用点
/// 展开——数字与比较的出处各只有一个。
pub(crate) fn is_fused(failures: u32) -> bool {
    failures >= MAX_COMPACT_FAILURES
}
/// 喂给摘要器时单条工具输出的字符上限（OpenCode `compaction.ts:100`）。
/// 这条不是为了省字：摘要请求本身也占窗口，不封顶就是"为了压缩而先撑爆"。
pub(crate) const TOOL_OUTPUT_MAX_CHARS: usize = 1_250;
/// overflow 应急递减带（OpenCode `compaction.ts:95`，刀④d 用）。
pub(crate) const SHRINK_STEPS: [f64; 3] = [0.7, 0.5, 0.35];

/// 压到有效阈值的这一成（**我们的裁定**，S4 · specs/11 刀① 起写死）。
/// 两家参照都没有这颗数；它不是一个用户能负责地填的量——填 100% 等于没压，
/// 填 10% 等于每轮重压一次。50% 是"压一次能撑一段"的经验值。
pub(crate) const TARGET_PCT: u64 = 50;

/// 摘要之外保留多少 token 的近期原文（OpenCode `compaction.ts:197` `keep: 15_000`
/// 逐字）。S2 期间曾是设置项，现在写死。
pub(crate) const KEEP_TOKENS: u64 = 15_000;

/// 七节节名（`prompts/compact/template.md` 里写的那七个，顺序一致）。
/// 唯一出处：模板与验收都读它（判据 SS20c），漂一个字 = 摘要永远"缺节"。
pub(crate) const SECTIONS: [&str; 7] = [
    "目标",
    "约束与要求",
    "已定决策",
    "工作状态",
    "下一步",
    "相关文件",
    "重要上下文",
];
/// 「工作状态」的三个子节（同样两边共用）。
pub(crate) const SUBSECTIONS: [&str; 3] = ["已完成", "进行中", "受阻"];

/// 一次判定要吃的所有读数（**纯函数**，调用方负责把它们凑齐）。
pub(crate) struct PlanInput<'a> {
    /// 当前要发出去的那份消息列（组装侧已经切好的窗口——本模块不猜它从哪来）。
    pub messages: &'a [Message],
    /// 有效阈值（`context_budget::effective_threshold` 那颗）。
    /// `None` = 窗口未知 ⇒ **不压**（裁定 A 在这一层的同一条姿态）。
    pub threshold: Option<u64>,
    /// 界面上看到的占用数（锚点 = provider 上报 ?? 全量估算）。
    pub used: u64,
    /// 目标比（百分数；设置项 `context.compactTargetPct`，缺省那颗数在
    /// `settings/context.rs`，本模块不复制第二份）。
    pub target_pct: u64,
    /// 近期原文保留额度（token；设置项 `context.compactKeepTokens`，同上）。
    pub keep_tokens: u64,
    /// 该会话已累计的连续失败次数（熔断判据的输入）。
    pub failures: u32,
    /// **人主动按的那一格**（S2 刀⑦：`/压缩` 手动通路）。它只让开两条"线"，
    /// 不让开任何一条"形状"——理由逐条写在 [`plan`] 里那两处。
    pub manual: bool,
}

/// 判定结果：`reason` = 为什么不动 / 动了，`cut` = 切在哪一条。
///
/// 这里**故意不给"压完会有多大"的预估**：摘要文本要等模型写完才存在，预估数要么
/// 骗人（写进日志的数不是实测），要么变成没人读的死字段。实测的 `from_tokens` /
/// `to_tokens` 由 [`deep`] 在落账那一刻用同一把 `estimate_tokens` 算出来。
#[derive(Debug)]
pub(crate) struct Plan {
    pub reason: &'static str,
    /// 切点：`messages[..cut]` 交给摘要器，`messages[cut..]` 原文保留。
    pub cut: usize,
}

impl Plan {
    fn stop(reason: &'static str) -> Plan {
        Plan { reason, cut: 0 }
    }
}

/// 摘要每一节按多少 token 占位（[`summary_reserve`] 的乘数）。
///
/// 200/节**是我们拍的**（两家参照里都没有这颗数：OpenCode 直接让摘要占固定额度，
/// ZCode 用阈值减缓冲）。它不单独做成设置项——它不是一个用户看得见的量，只是
/// "目标位要先给摘要腾地方"这件事的换算尺；界面上要调的是目标比那一成
/// （`context.compactTargetPct`），腾出来的地方自然跟着变。
const SUMMARY_RESERVE_PER_SECTION_TOKENS: u64 = 200;

/// 纯判定：要不要压、切在哪。任何一步不满足都如实带回 `reason`（观测面读它）。
///
/// `manual = true`（人主动按 `/压缩`）只让开**下面两条"线"**，三条"形状"一条不让：
/// 线是"值不值得自动花这一趟"，形状是"压了会不会把上下文压坏"——后者对人同样成立，
/// 所以一轮都凑不齐时（`too-few-rounds`）没有可压的一段（`nothing-to-summarize`）
/// 手动照旧如实拒。窗口未知（`unknown-window`）也照旧拒：目标位算不出来，
/// 强行压一次只能靠猜，压完仍然可能超窗 = 假装有进展（裁定 A 的同一条姿态）。
pub(crate) fn plan(input: &PlanInput<'_>) -> Plan {
    let msgs = input.messages;
    if input.threshold.is_none() {
        return Plan::stop("unknown-window");
    }
    let threshold = input.threshold.unwrap_or(0);
    if !input.manual && input.used < threshold {
        return Plan::stop("below-line");
    }
    // 这张表管的是"自动路径别连着白花模型往返"（没人看着时连败三次就该停手）；
    // 手动那一下正是"人在看着、要它再试"。计数器本身照旧走（失败仍 record、
    // 成功仍 clear）——让开的只是这一格的判定，不是它的账。
    if !input.manual && is_fused(input.failures) {
        return Plan::stop("fused");
    }
    // 组数与"有没有 assistant"（ZCode `hasEnoughMessagesToCompact`：`rounds >= 2 &&
    // 有 assistant`）——一轮都凑不齐就压，等于把唯一的上下文换成一份空洞摘要。
    if !enough_rounds(msgs) {
        return Plan::stop("too-few-rounds");
    }
    // 目标位**先减掉摘要自己要占的位子**再去切（[`summary_reserve`]）。
    let budget = target_budget(threshold, input.target_pct)
        .saturating_sub(summary_reserve())
        .max(1);
    let cut = choose_cut(msgs, budget, input.keep_tokens);
    if cut == 0 {
        return Plan::stop("nothing-to-summarize");
    }
    Plan {
        reason: "planned",
        cut,
    }
}

/// 摘要自身要占多少位子：七节 × 每节 [`SUMMARY_RESERVE_PER_SECTION_TOKENS`]。
/// 为什么这颗数要从目标位里**先扣**而不是事后核对：模型写的摘要长短不可控，留一整
/// 节的量是保守写法；不扣就是"压到 50%"实际压到 50% 再加一份摘要 ⇒ 下一次请求照旧
/// 到线 ⇒ 每一轮都再压一次（这是"压不到位"里最省钱也最难发现的一种）。
pub(crate) fn summary_reserve() -> u64 {
    SECTIONS.len() as u64 * SUMMARY_RESERVE_PER_SECTION_TOKENS
}

/// 目标位 = 有效阈值 × 这一成。`target_pct` 由设置面限制在 1–100，这里再兜一次
/// （纯函数不许被越界读数骗出 0 阈值 ⇒ 永远压不到位）。
pub(crate) fn target_budget(threshold: u64, target_pct: u64) -> u64 {
    threshold.saturating_mul(target_pct.clamp(1, 100)) / 100
}

/// 至少两组往返、且里面有 assistant，才值得压（ZCode `manual.ts:76-83` 同判据）。
fn enough_rounds(messages: &[Message]) -> bool {
    let mut rounds = 0usize;
    let mut has_assistant = false;
    for m in messages {
        match m {
            // 一条 user 开一组（我方日志里一轮 = user → assistant → 工具对，
            // 与 ZCode "assistant 发起的一组"同一计数口径，只是锚在不同角色上）。
            Message::User { .. } => rounds += 1,
            Message::Assistant { .. } => has_assistant = true,
            _ => {}
        }
    }
    rounds >= 2 && has_assistant
}

/// 切点选择：从最新往回累计原文，装到「近期保留额度」用完为止；再往前推到
/// **一条 user 的边界**（绝不从半轮中间开始发，与内核 `tail_user_start` 同一条纪律）；
/// 最后按目标位再收一次（近期原文本身就把预算吃满时，丢最旧的原文、保最新的）。
/// 返回值 = 第一条保留消息的下标；0 = 无可压（全留）。
fn choose_cut(messages: &[Message], budget: u64, keep_tokens: u64) -> usize {
    let n = messages.len();
    let mut acc = 0u64;
    let mut cut = n;
    for i in (0..n).rev() {
        let size = estimate_tokens(&messages[i..i + 1]);
        if acc.saturating_add(size) > keep_tokens {
            break;
        }
        acc += size;
        cut = i;
    }
    // 推到一个 user 边界：cut 落在 assistant/工具对上时，把它前进到下一条 user。
    cut = advance_to_user(messages, cut);
    // 目标位那一刀：近期原文已经超过预算，就从它里面丢最旧的（永远留最后一条 user）。
    // 这一刀**必须能在 keep 额度装下全部历史之后仍然切下去**——否则"压到 50%"这颗设置
    // 项在会话还没长到 15K 时形同不存在（用例
    // `a_tight_budget_drops_oldest_kept_messages_instead_of_doing_nothing` 钉的就是这一档）。
    // ⚠ `next >= n` 必须 break 而不是赋值：再往前没有 user 边界时 `advance_to_user`
    // 返回末尾，照抄下去会把 cut 推到 n ⇒ 保留段变空 ⇒ 明明有一大段可压却报
    // "nothing-to-summarize"（夹具测不出来、真会话里表现为"压缩从不工作"）。
    while cut + 1 < n && estimate_tokens(&messages[cut..]) > budget {
        let next = advance_to_user(messages, cut + 1);
        if next >= n || next <= cut {
            break;
        }
        cut = next;
    }
    // cut == 0 = 保留额度与目标位都装得下整段历史 ⇒ 没有头部可摘，如实报无可压。
    if cut == 0 {
        return 0;
    }
    // `advance_to_user` 找不到边界时返回 `n` ⇒ 保留段为空。空保留段等于"用户刚才那句
    // 话也被压进摘要"，宁可什么都不做：报 0（调用方如实回 "nothing-to-summarize"）。
    // cut < n 时 messages[cut] 必是 user（上面那一推保证），不需要再查一遍。
    if cut >= n {
        return 0;
    }
    cut
}

fn is_user(m: &Message) -> bool {
    matches!(m, Message::User { .. })
}

/// 从 `from` 起找第一条 user 的下标；找不到（后面全是过程消息）就停在末尾——
/// 那意味着"全部保留"，等价于不压。
fn advance_to_user(messages: &[Message], from: usize) -> usize {
    (from..messages.len())
        .find(|&i| is_user(&messages[i]))
        .unwrap_or(messages.len())
}

/// 主层的连续失败表（熔断的输入）。
///
/// 为什么它不待在 [`Compactor`](crate::compact::deep::Compactor) 里：这张表有两个读者——
/// 写它的是主层自己（失败 +1、压成清零），而刀⑤ 的滑窗保护网要在**另一条通路上**读
/// "此刻熔断了没"来决定开不开。计数留在 `Compactor` 里，保护网就得持有 `Compactor`，
/// 而后者是 runner 的 step 边界依赖（带摘要客户端、带设置）——为一个只读布尔拖来一整套。
/// 与刀④a/④c 把 `AppLlmSummarizer` 提到组装点同一条理由：**共用件不归属任何一方**。
/// 构造点因此在 `state/mod.rs`，两个使用方各收一枚 `Arc`。
///
/// 进程内、不落盘（沿用 `Compactor` 原来那段理由）：它管的是"这条通路此刻是不是坏了"，
/// 跨重启没有意义——重启后设置可能都换了，该重新给它一次机会。
#[derive(Default)]
pub(crate) struct Failures {
    map: Mutex<HashMap<String, u32>>,
}

impl Failures {
    /// 该会话当前累计的连续失败次数（没有记录 = 0）。
    pub(crate) fn of(&self, session_id: &str) -> u32 {
        *self
            .map
            .lock()
            .expect("compact failures poisoned")
            .get(session_id)
            .unwrap_or(&0)
    }

    /// 记一次失败并返回累计数（观测面要报"这是第几次"）。
    pub(crate) fn record(&self, session_id: &str) -> u32 {
        let mut map = self.map.lock().expect("compact failures poisoned");
        let n = map.entry(session_id.to_string()).or_insert(0);
        *n += 1;
        *n
    }

    /// 压成了 ⇒ 整格清零（熔断随之解除，保护网下一轮自动退场）。
    pub(crate) fn clear(&self, session_id: &str) {
        self.map
            .lock()
            .expect("compact failures poisoned")
            .remove(session_id);
    }
}

/// 最近一次**被供应商按超窗拒收**的占用读数（S2 刀⑧ · 台账 ⓜ）。
///
/// 为什么必须有这张表：触发线的分子是锚点 = 会话日志里最后一条 `LlmUsage`
/// （`context_budget::anchor`），而**被拒的那一次请求不会产生用量上报**——供应商
/// 是在处理它之前就把门摔上的。于是那一轮之后：历史照长、请求照被拒、日志里那颗
/// 读数永远停在"被拒之前"的那个数 ⇒ 到线判定永远读不到已经超了 ⇒ **整条压缩面
/// 自锁**（界面上的样子是"每一轮都失败，而压缩开关明明开着"）。100 轮长跑实测到
/// 这一版的形状：41 个轮全部失败、微压缩与主层各 0 条落账。
///
/// 写它的是发送侧（只有它知道自己这一次被拒了、以及那一次发了多少），读它的是
/// 压缩侧（判要不要起步）——**两个使用方，所以它不归任何一方**，构造点在组装点，
/// 与 [`Failures`] 同族同一条纪律。
///
/// 进程内、不落盘（沿用 [`Failures`] 那段理由）。⚠ 要说破这一档的代价：崩溃重放
/// 之后这张表是空的，但那只退化成"再被拒一次、再记上"，自愈路径仍然成立——
/// 而**永远不会**再自锁，因为被拒现场每次都重新记账。
#[derive(Default)]
pub(crate) struct Overflow {
    map: Mutex<HashMap<String, u64>>,
}

impl Overflow {
    /// 记一次被拒的占用（同会话多次被拒留最坏那个：窗口在被压小之前只会更大，
    /// 取小而丢掉更早那次更大的证据 = 又把自锁请回来）。
    pub(crate) fn note(&self, session_id: &str, tokens: u64) {
        let mut map = self.map.lock().expect("compact overflow table poisoned");
        let cell = map.entry(session_id.to_string()).or_insert(0);
        *cell = (*cell).max(tokens);
    }

    /// 该会话最近一次被拒时的占用读数（没有记录 = `None`）。
    pub(crate) fn of(&self, session_id: &str) -> Option<u64> {
        let map = self.map.lock().expect("compact overflow table poisoned");
        map.get(session_id).copied()
    }

    /// **喂给占用分子的取数口**（S3 刀④a）：没有记录 = 0 = "这一档不参与取最大"。
    /// 这一格裁定只做一次：改前压缩面与（新接入的）组装点各写一遍
    /// `of(..).unwrap_or(0)`，多一个消费者就多一个出处——而"缺席当 0"写错的形态
    /// 是"缺席当无穷"，那一档会把触发线永久顶在线上，正是 ⓜ 要治的病。
    pub(crate) fn reading(&self, session_id: &str) -> u64 {
        self.of(session_id).unwrap_or(0)
    }

    /// 压成了 ⇒ 窗口已经变小，那颗"上次发出去被拒"的数不再描述当前窗口（不清就等于
    /// 让一次旧的拒收把这一层永久顶在线上方 = 刀④ 那条"每个 step 重压一次"的坑）。
    pub(crate) fn clear(&self, session_id: &str) {
        self.map
            .lock()
            .expect("compact overflow table poisoned")
            .remove(session_id);
    }

    /// 表里现有几格（只给测试用：判"没有会话身份那一次有没有多建一格"——光看某个键
    /// 读回 `None` 判不出这件事，那个键本来就恒为 `None`）。
    #[cfg(test)]
    pub(crate) fn entries(&self) -> usize {
        self.map
            .lock()
            .expect("compact overflow table poisoned")
            .len()
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod tests_deep;

/// 一份**七节齐全**的摘要文本（测试专用，判据侧与接线侧共用一个出处）。
///
/// 为什么放在被测模块里而不是各测试文件写一份：它的形状由 [`SECTIONS`] +
/// [`SUBSECTIONS`] 决定。两处各手抄一份的话，节名表一改就会让某一处红在"缺节"上，
/// 而那条红看起来像被测对象坏了。
#[cfg(test)]
pub(crate) fn sample_summary() -> String {
    let mut s = String::new();
    for name in SECTIONS {
        s.push_str(&format!("## {name}\n关于{name}的一句话\n\n"));
        if name == "工作状态" {
            for sub in SUBSECTIONS {
                s.push_str(&format!("### {sub}\n（无）\n"));
            }
            s.push('\n');
        }
    }
    s
}
