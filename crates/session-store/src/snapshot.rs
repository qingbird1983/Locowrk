//! 快照与 schema 演进（DESIGN.md §9.5）。
//!
//! 快照 state 用 `Value` 而非强类型，便于演进；加载时逐级 upcast 到当前版本。
//!
//! **定性 = 预留待接，不是死码也不是缺陷**（审计 M-20 / 计划 D3，v2.148 批 D
//! 查证并标注）：DESIGN §9.5 与设计纪律 5（「快照带 schema_version upcaster」）
//! 明写了这套机制，实现与测试都在位，但**磁盘面从未接上**——现网恢复走
//! `persist.rs` 的逐行 JSONL 回放（会话日志是唯一真相源，快照只负责加速），
//! 所以 `snapshot_at` / `from_snapshot` / `Upcasters` 三个入口的生产调用点
//! 为零（`cargo` 层面它们是本 crate 对外 API，编译器不告警）。
//!
//! **预留件清单**：snapshot_at, from_snapshot, Upcasters
//!
//! （批 Q7 · v2.191 给这行加了机器闸：`scripts/check-reserved.mjs` 按清单逐项验"生产零调用点"，
//! 一旦接上线而标注没摘就判红。上面那段自然语言叙述是全仓这个形态的正身，本批两处新标注
//! （`memory-tree` 的 `main_prompt` 与 `index`）就是照它学的。）
//!
//! 接上的那天要做的事（留给未来批次，不在本轮凭空接线）：`sessions/<id>.snapshot`
//! 落盘 + `load_session` 改「快照基线 + 其后事件回放」，并给 schema 版本演进
//! 补真实 upcaster 链（当前只有恒等的 v1→v2）。
//!
//! ⚠ 别和这套混起来：`Session::snapshots`（session.rs，**crate 内可见**）是
//! 内存态的语序折叠表，`fork`/`rewind_to`/`state_at` 每轮都在读写它 = 活的。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::StoreError;
use crate::session::{Session, SessionState};

/// 当前快照 schema 版本。演进 = 旧版本 upcaster 逐级升档；只增不减。
/// v2（批 D）：Message::User +parts（serde default）——v1 state 反序列化
/// 时缺字段天然补空，upcaster 恒等（版本号记录格式演进这件事本身）。
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 2;

/// v1 → v2 upcaster（批 D parts 字段）：state 结构无需改写——
/// `Message::User.parts` 带 serde default，反序列化侧自动补空。
#[derive(Default)]
pub struct V1ToV2Upcaster;

impl Upcaster for V1ToV2Upcaster {
    fn source_version(&self) -> u32 {
        1
    }
    fn upcast(&self, state: Value) -> Value {
        state
    }
}

/// 可持久化快照（§9.5）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub schema_version: u32,
    pub seq: usize,
    pub state: Value,
}

/// 把 `source_version` 版本的 state 升到 v+1。
pub trait Upcaster: Send + Sync {
    fn source_version(&self) -> u32;
    fn upcast(&self, state: Value) -> Value;
}

/// upcaster 注册表：source_version → upcaster。
#[derive(Default)]
pub struct Upcasters {
    chain: BTreeMap<u32, Box<dyn Upcaster>>,
}

impl Upcasters {
    pub fn register(&mut self, upcaster: Box<dyn Upcaster>) {
        self.chain.insert(upcaster.source_version(), upcaster);
    }

    /// 逐级升档到当前版本（§9.5 的 load_snapshot 循环）。
    /// 低于缺口（缺 upcaster）或高于当前版本都报错——快照不可降级。
    pub fn to_current(&self, mut snap: Snapshot) -> Result<Snapshot, StoreError> {
        let current = SNAPSHOT_SCHEMA_VERSION;
        while snap.schema_version < current {
            let up = self
                .chain
                .get(&snap.schema_version)
                .ok_or(StoreError::SchemaGap {
                    found: snap.schema_version,
                    target: current,
                })?;
            snap.state = up.upcast(std::mem::take(&mut snap.state));
            snap.schema_version += 1;
        }
        if snap.schema_version > current {
            return Err(StoreError::TooNew {
                found: snap.schema_version,
                current,
            });
        }
        Ok(snap)
    }
}

impl Session {
    /// 物化可持久化快照（§9.5）。
    pub fn snapshot_at(&self, seq: usize) -> Snapshot {
        Snapshot {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            seq,
            state: serde_json::to_value(self.state_at(seq)).expect("session state serializes"),
        }
    }

    /// 从快照恢复会话：升档 → 反序列化 → 以快照状态为 0 号基线。
    /// 恢复后的会话语序从 0 重排，快照之前的事件不随之恢复
    /// （完整日志另存；快照只负责加速 replay，§6.2）。
    pub fn from_snapshot(snap: Snapshot, upcasters: &Upcasters) -> Result<Self, StoreError> {
        let snap = upcasters.to_current(snap)?;
        let state: SessionState = serde_json::from_value(snap.state)
            .map_err(|e| StoreError::Invalid(format!("snapshot state: {e}")))?;
        let mut snapshots = BTreeMap::new();
        snapshots.insert(0, state);
        Ok(Self {
            log: Vec::new(),
            cursor: 0,
            snapshots,
            schema_version: SNAPSHOT_SCHEMA_VERSION,
        })
    }
}

#[cfg(test)]
mod tests;
