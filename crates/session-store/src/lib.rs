//! 会话事件日志：唯一真相源（DESIGN.md §4、§6）。
//!
//! 计划模块：`event`（SessionEvent 定义）、`session`（追加/投影/fork）、
//! `snapshot`（每 N 事件快照 + schema_version upcaster 演进）、
//! `compensation`（副作用回退：Pure / Compensable / Irreversible 三级）。
//!
//! 纪律：事件只追加不删除，回退 = 移动游标 + replay；
//! 补偿工具必须幂等，用 `call_id` 做去重键。
