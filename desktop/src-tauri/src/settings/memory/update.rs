//! 记忆分区的校验与应用（台账 T15 从 `settings/update.rs` **机械搬移**而来）。
//!
//! 为什么单独成文件而不是并回 `mod.rs`：`mod.rs` 现值 293 行，本段 126 行搬进去
//! 就是 419 行 = 新增超限件（AGENTS §3「单文件建议 ≤400」+ `check_length` 只拦新增）。
//! 按纪律做三问自检：本区只管"记忆设置"一件事 ✓、名字概括得住 ✓、但**改一档常常
//! 要在一整块 300+ 行的字段清单里找那一档**（第三问为"否"）⇒ 拆成目录式子模块。
//!
//! 搬移只做三条变换，逐条都能被脚本验：
//!   1. `next.memory.` → `self.`
//!   2. 缩进减一层（原来在 `if let Some(u) = memory { … }` 块里）
//!   3. `super::memory::` → `super::`（本文件的 `super` 就是 `memory` 模块本身）
//! 其余逐字节不动，包括注释里那句"本文件 393 行"——它是批 Q4 当时的事实，历史记录
//! 不改写；现在的实情写在上面这段头注里。
//! 等价性证据 = 既有 `settings/tests/memory.rs` 一字未改仍然绿 + 行集合差脚本为空
//! （脚本与读数见提交正文；判据形态沿用批 Q6 的 `developer.rs` 先例）。

use super::{MemorySettings, MemoryUpdate};

impl MemorySettings {
    /// 校验 + 应用（分区自己收口，编排层只留一行转发）。签名吃**消费型值**：
    /// 本区有 6 个 `String` 字段是"从 u 里搬出来落值"，改成 `&u` 就得给每一处
    /// 补 `.clone()`——那是新写的代码而不是搬移，等价性从此只能靠人读。
    pub(crate) fn apply_update(&mut self, u: MemoryUpdate) -> Result<(), String> {
        if let Some(e) = u.enabled {
            self.enabled = e;
        }
        if let Some(k) = u.top_k {
            if !(1..=20).contains(&k) {
                return Err(format!("记忆注入条数 {k} 超出范围（1–20）"));
            }
            self.top_k = k;
        }
        // M2 记忆树整理三键（树 v2 §4.3/§5.2/§15）：节奏白名单 +
        // 增量块区间 + 记忆管理器模型（空串按清除处理，回落同主模型）
        if let Some(p) = u.organize_pacing {
            if !super::ORGANIZE_PACING_VALUES.contains(&p.as_str()) {
                return Err(format!("整理节奏 {p:?} 非法（可选 batch / incremental）"));
            }
            self.organize_pacing = p;
        }
        if let Some(n) = u.organize_chunk_tokens {
            if !super::ORGANIZE_CHUNK_TOKENS_RANGE.contains(&n) {
                return Err(format!(
                    "增量块 {n} 超出范围（{}–{} token）",
                    super::ORGANIZE_CHUNK_TOKENS_RANGE.start(),
                    super::ORGANIZE_CHUNK_TOKENS_RANGE.end()
                ));
            }
            self.organize_chunk_tokens = n;
        }
        // 记忆管理器模型（空串 = 清除，回落同主模型；与 base_url/api_key
        // 同约定——双层 Option 的 null 在 JSON 里无法表达清除）
        if let Some(m) = u.memory_manager_model {
            self.memory_manager_model = (!m.trim().is_empty()).then_some(m);
        }
        if let Some(p) = u.memory_manager_provider {
            self.memory_manager_provider = (!p.trim().is_empty()).then_some(p);
        }
        // M4 尾巴策略 + collapse（树 v2 §7.2/§8.3/§15）：策略白名单 +
        // 窗口轮数区间 + 收起阈值区间
        if let Some(p) = u.tail_strategy {
            if !super::TAIL_STRATEGY_VALUES.contains(&p.as_str()) {
                return Err(format!("尾巴策略 {p:?} 非法（可选 boundary / window）"));
            }
            self.tail_strategy = p;
        }
        if let Some(n) = u.tail_window_turns {
            if !super::TAIL_WINDOW_TURNS_RANGE.contains(&n) {
                return Err(format!(
                    "固定窗口 {n} 超出范围（{}–{} 轮）",
                    super::TAIL_WINDOW_TURNS_RANGE.start(),
                    super::TAIL_WINDOW_TURNS_RANGE.end()
                ));
            }
            self.tail_window_turns = n;
        }
        if let Some(e) = u.collapse_enabled {
            self.collapse_enabled = e;
        }
        if let Some(p) = u.collapse_threshold_pct {
            if !super::COLLAPSE_THRESHOLD_PCT_RANGE.contains(&p) {
                return Err(format!(
                    "自动收起阈值 {p}% 超出范围（{}–{}%）",
                    super::COLLAPSE_THRESHOLD_PCT_RANGE.start(),
                    super::COLLAPSE_THRESHOLD_PCT_RANGE.end()
                ));
            }
            self.collapse_threshold_pct = p;
        }
        if let Some(e) = u.files {
            self.files = e;
        }
        if let Some(e) = u.repo_context {
            self.repo_context = e;
        }
        // S1 刀④b 三枚行为开关里的两枚（第三枚 = 上面那 `repoContext`）。两枚都是
        // **只出不进**的档：关掉只停掉"这一层进上下文"，一个文件都不删、管理页照旧全量，
        // 所以这里没有"要不要先清空"的分支——那种分支会把一个显示开关变成数据破坏动作。
        if let Some(e) = u.global_layer {
            self.global_layer = e;
        }
        if let Some(e) = u.project_layer {
            self.project_layer = e;
        }
        if let Some(e) = u.instruction_files {
            self.instruction_files = e;
        }
        // 指令文件两档链预算（v2.188 · 队列 Q4）：区间端点只有 `INSTRUCTION_KIB_RANGE`
        // 一个出处，校验体在 `settings/memory.rs::check_instruction_kib`（本文件
        // 393 行，就地写两遍会顶成新增超限件 ⇒ 抽函数，见那里的头注），
        // **标签各写各的**：出界文案必须点名是哪一档。
        if let Some(n) = u.instruction_global_kib {
            super::check_instruction_kib("全局", n)?;
            self.instruction_global_kib = n;
        }
        if let Some(n) = u.instruction_project_kib {
            super::check_instruction_kib("项目", n)?;
            self.instruction_project_kib = n;
        }
        if let Some(e) = u.fact_cards {
            self.fact_cards = e;
        }
        if let Some(n) = u.fact_card_max {
            if !super::FACT_CARD_MAX_RANGE.contains(&n) {
                return Err(format!(
                    "整理事实卡上限 {n} 超出范围（{}–{} 张）",
                    super::FACT_CARD_MAX_RANGE.start(),
                    super::FACT_CARD_MAX_RANGE.end()
                ));
            }
            self.fact_card_max = n;
        }
        // 学习档（v2.200 · 队列 Q9）：开关直通，四档各走一次区间校验。
        // 校验体抽在 `settings/memory.rs::check_learn_weight`（同 Q4 那两档的
        // 理由：区间端点只许一个出处）；这里**四遍各写各的中文档名**——出界
        // 文案必须点名是哪一档，且不接受用字符串当字段路由那种"看起来省行数"
        // 的形状（省下的行数由排障时多读的三层 match 还回去）。
        if let Some(e) = u.learn_enabled {
            self.learn_enabled = e;
        }
        if let Some(n) = u.relevance_weight {
            super::check_learn_weight("相关性", n)?;
            self.relevance_weight = n;
        }
        if let Some(n) = u.strength_weight {
            super::check_learn_weight("强度", n)?;
            self.strength_weight = n;
        }
        if let Some(n) = u.freshness_weight {
            super::check_learn_weight("热度", n)?;
            self.freshness_weight = n;
        }
        if let Some(n) = u.recency_weight {
            super::check_learn_weight("新鲜度", n)?;
            self.recency_weight = n;
        }
        Ok(())
    }
}
