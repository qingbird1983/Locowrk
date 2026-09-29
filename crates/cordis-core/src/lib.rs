//! Cordis 五支柱（DESIGN.md §3）。
//!
//! 计划模块：`context`（TypeId 服务载体）、`effect`（可逆修改 + LIFO 撤销链）、
//! `fiber`（插件六态生命周期）、`scope`（单进程多 Agent 隔离）、
//! `event_bus`（emit / parallel / serial / waterfall 四种分发语义）、
//! `runtime`（依赖图 + 拓扑调度）。
//!
//! 纪律：所有对 `Context` 的修改必须走 `effect()` 并携带逆操作（Disposer）。
