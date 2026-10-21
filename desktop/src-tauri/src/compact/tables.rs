//! 主层深度压缩的**两张共用表**（从 `compact.rs` 拆出 · specs/12 刀②）。
//!
//! 为什么它们单独成文件：主层压缩的**判据面**（要不要压、切在哪、哪一段进摘要）
//! 住在 `compact.rs`，那两张表住在别处——它们是**状态**，不是判据。混在一处时
//! `compact.rs` 头注那句「本文件只放纯判据」是假的（读者会以为那里全是判据）。
//!
//! 两张表各自为什么**不归任何一方**：写它的是主层自己，读它的也有别人
//! （观测面 / 组装点 / 测试）。共用件不归属任何一方 ⇒ 构造点在组装点
//! （`state/mod.rs`），两个使用方各收一枚 `Arc`（同 `AppLlmSummarizer` 提到
//! 组装点那条纪律）。
//!
//! 拆出后 `compact.rs` 侧用 `pub(crate) use tables::{Failures, Overflow};`
//! 重新导出 ⇒ **调用方的路径一个字没改**（`crate::compact::Failures` 照旧）。

use std::collections::HashMap;
use std::sync::Mutex;

/// 主层的连续失败表（熔断的输入）。
///
/// 为什么它不待在 [`Compactor`](super::deep::Compactor) 里：这张表有两个读者——
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
