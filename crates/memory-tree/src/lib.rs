//! 记忆树（捆稻草模型，《Memory Tree Design v2》）：会话事件流的**索引**
//! 投影——原文永不进树，树只记 `event_range`，坏了删掉重建（§1.3）。
//!
//! M2 影子运行（路线表）：本 crate 只提供领域件——章节结构 / 树操作 /
//! 反向索引 / 阈值触发决策 / 记忆管理器材料与输出校验 / 规则切分降级 /
//! (workspace, session) 键位存储。请求组装一行不动（§11.2 阶段 1），
//! 组装器（ContextAssembler）属 M4。
//!
//! 模块（树 v2 §11.3 布局 + 存储拆分）：
//! - `chapter`：MemoryChapter（L0 标题 / L1 摘要 / L2 引用）与草稿
//! - `tree`：MemoryTree（章节表 + 已整理游标 + 在途任务 + 反弹熔断 + 历史）
//! - `index`：EventToChapterIndex 反向索引（事件位 → 所属章节）
//! - `trigger`：阈值 / 增量触发决策（纯函数；占用锚点优先 LlmUsage §4.2）
//! - `material`：事件切片 → 记忆管理器转录材料（`[N]` 行号标记）与估算
//! - `manager`：记忆管理器客户端契约 + 提示词（两层目录 §6.2）+ 输出解析
//!   校验（不合规 → 规则切分 §5.4）+ 摘要逐字保留纪律在提示词内（§5.6）
//! - `summary`：摘要补写的树侧写路径（队列 Q8c：规则切分产物欠的那次 L1 补写）
//! - `storage`：`memory-tree/<scope>/<session>.json` 投影持久化（原子写）

pub mod chapter;
pub mod index;
pub mod manager;
pub mod material;
pub mod storage;
pub mod summary;
pub mod tree;
pub mod trigger;

pub use chapter::{ChapterDraft, ChapterId, EventId, MemoryChapter, sanitize_draft};
pub use index::EventToChapterIndex;
pub use manager::{
    FACT_VALUE_MAX_CHARS, FactCard, MemoryManagerClient, MemoryManagerError, MemoryManagerUsage,
    RUN_SUMMARY_MAX_CHARS, fact_cards, fact_request, family_of, main_prompt, manager_prompt,
    parse_drafts, parse_summaries, rule_split_drafts, run_summary, summarizer_prompt,
};
pub use material::{MaterialItem, MaterialKind, build_items, transcript, user_count};
pub use storage::{TreeStore, TreeSummary};
pub use summary::ChapterSummary;
pub use tree::{
    MemoryTree, OrganizeRecord, PendingTask, ReboundDecision, ReboundState, RunSummary,
};
pub use trigger::{AnchorSource, OrganizeReason, Pacing, TriggerInput, should_auto_organize};
