//! Q1a（L0 注入侧可观测性）验收判据 —— 对应《Memory Learning Mechanism》§2.4，
//! 外加四条稿子没写但必须钉住的：**去重口径**、**「库不可用」不许冒充「库是空的」**、
//! **接线前后召回逐字段相等（零行为变更的机器证明）**、**未接线不许自己建库**。
//!
//! 全部驱动 `MemoryStore::recall()` **生产本体**，不是手搓事件再测自己。

use std::path::PathBuf;
use std::sync::Arc;

use crate::memory::MemoryStore;
use crate::recall_log::{RecallLog, RecallRow};

/// 拿到手就建好的空目录。
///
/// ⚠ `testutil::unique_dir` 只**拼**路径不建目录（它的契约是把"目录不存在"留给调用方），
/// 而 `RecallLog::at` 同样不碰盘——首条 open 会在不存在的目录里建 `usage.sqlite3`
/// 并直接失败（实测报错 `unable to open database file`）。**修法在测试侧**：给
/// `RecallLog` 加 `create_dir_all` 会把「数据根没建好」这个真实故障吞成静默成功。
fn temp_dir(tag: &str) -> PathBuf {
    let d = crate::testutil::unique_dir(&format!("recalllog-{tag}"));
    std::fs::create_dir_all(&d).expect("create recall_log temp dir");
    d
}

/// 测试解析器：`ws-session` 归属 `F:/ws/a`，`other-session` 归属另一根，其余未归属。
fn test_resolver() -> crate::workspace::WorkspaceResolver {
    Arc::new(|sid: &str| match sid {
        "ws-session" => Some("F:/ws/a".to_string()),
        "other-session" => Some("F:/ws/b".to_string()),
        _ => None,
    })
}

/// 记忆库与观测表放在**同一目录**——它就是生产现场的位置关系（`state/boot.rs` 用
/// `root.join("usage.sqlite3")`，`MemoryStore` 用 `root/memory.sqlite3`）。
fn store_with_log(tag: &str, seed: &[(&str, &str, &str)]) -> (MemoryStore, Arc<RecallLog>) {
    let dir = temp_dir(tag);
    let log = RecallLog::at(dir.join("usage.sqlite3"));
    let store = MemoryStore::new(&dir, test_resolver()).with_recall_log(Arc::clone(&log));
    for (ws, key, value) in seed {
        store.write(ws, key, value, "t", "explicit").unwrap();
        // 影子那一行改用**生产里唯一还写内容列的入口**造（S1 刀③b-2 之后 `write` 不发
        // SQL）。这一步不能省：学习档 `recall_relevant` 仍按库取候选，库里没行时它的
        // 候选集是空的 ⇒ 整簇变成"在空集上断言"。键位与 `write` 同一个归一函数，
        // 否则真源与影子落在两个键位上（那是本批判据要防的形状，不是夹具该造的）。
        store
            .reconcile_upsert(
                &crate::workspace::scope_key(Some(ws)),
                key,
                value,
                "t",
                "explicit",
                crate::timeutil::now_secs(),
            )
            .unwrap();
    }
    (store, log)
}

fn rows(log: &RecallLog) -> Vec<RecallRow> {
    log.recent(50).unwrap()
}

/// 把召回交付压成可比较的元组（不依赖 `MemoryEntry` 的 derive 形状）。
fn shape(entries: &[kernel_agent::MemoryEntry]) -> Vec<(String, String, String, String, i64)> {
    entries
        .iter()
        .map(|e| {
            (
                e.key.clone(),
                e.value.clone(),
                e.tags.clone(),
                e.scope.clone(),
                e.updated_at,
            )
        })
        .collect()
}

/// `breakdown` 那一列的三档语义 + 去重先到先得 + 存量库补列（队列 Q10 / L4）。
mod breakdown;
/// 注入档埋点（Q1a 的十一条：三档形状 / 去重口径 / 零行为变更 / 不反噬被观测者）。
mod injection;
/// 派生指标口径（`stats()`：空块率与 `injection_yield` 的分子分母，Q1b 加的后四条）。
mod metrics;
