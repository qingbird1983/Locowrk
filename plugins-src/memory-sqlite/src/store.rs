//! SQLite 存储实现。连接按调用打开、用完即弃（host 侧 spawn_blocking 串
//! 调用，单文件库足够；busy_timeout 兜底并发），表结构惰性创建。
//!
//! **项目隔离（㉘）**：主键 = `(workspace, key)`——`workspace` 是调用会话
//! 归属的项目根（经宿主 vtable `session_context` 查询；`""` = 未归属）。
//! 同名 key 在不同项目下互不覆盖、互不可见（Context 文档 §4.6 隔离规则）。
//! 旧库（key 单主键、无 workspace 列）首次访问时迁移：整表重建，旧行归入
//! 未归属键位。宿主侧（desktop memory.rs）同持一份等价迁移——两侧建表
//! 形状一致（含宿主演进的 `source` 列），先跑谁结果都一样。
//!
//! 台账第三节 · 第二批第二刀（581 → 本文件留声明 + 一个原样未动的嵌套 mod）：五个面沉进
//! `store/`，`tools` 与 `undo` 靠 `pub(crate) use X::*;` 带出（glob 按**收窄后的可见性**
//! 再导出 `pub(crate)` 项，所以 `lib.rs` 的 `store::memory_*` 分发与测试里的同名引用一个字不用改）。
//! 五片 = `limits`（三个字符级上限）· `schema`（建表串 + 两条旧库迁移 + 开连接）·
//! `rows`（工具入口共用的读侧件与键位裁决）· `tools`（五个记忆工具入口）· `undo`（补偿入口）。
//! `mod mdsync` 整块留在本文件**没提成本级文件**：提成 sibling 要给 45 行统一减四个空格，
//! 那是把"逐字节等价"换成"缩进归一后等价"——本刀不做，代价（本文件不薄）与理由都登记在台账。
//! 上面那条 `pub(crate) use crate::WsCtx;` 是给被搬的 `super::WsCtx` 写法接根的：
//! 切片后各片的 `super` 是 `store` 而不是 crate root，加这一条就**不必改被搬文本的一个字**。

pub(crate) use crate::WsCtx;

mod limits;
mod rows;
mod schema;
mod tools;
mod undo;

pub(crate) use tools::*;
pub(crate) use undo::*;

/// F1 人类层双写（《Memory File-ization Design》§3）：sqlite 写/删成功后
/// 同步 `<数据根>/memory/` 下的 md。**失败不破契约**——sqlite 是召回真源，
/// md 是人读面；失败只以 `md_warning` 字段如实带回（F2 reconcile 可修复），
/// 总开关关 = 完全跳过。
///
/// ⚠ 这一段"sqlite 是召回真源"在 S1 刀② 之后已经不成立（宿主的读面全扳到 md 了），
/// 本刀（③a）只给 `sync_write` 加了一个 `ty` 参数把四类型带进 md；整个 mdsync 与
/// 它上面那句话随**刀③b**（六枚工具内部改道 md）一起消失——所以这里不给它做
/// `&FactEntry` 收口（宿主侧 `MemoryFiles::write_fact` 收了，因为它要继续活着）。
mod mdsync {
    use plugin_api::memlayout::{self, FactEntry};

    fn sync(f: impl FnOnce() -> Result<(), String>) -> Option<String> {
        f().err().map(|e| format!("md sync failed: {e}"))
    }

    /// `ty = None` = **调用方不知道分类**（undo 的载荷是从库里那一行取的，库里没有这一维）
    /// ⇒ 沿用现存文件里的那一行；文件也不在 = 未分类。这里不许写死 `""`：那会让一次
    /// undo 把四类型抹平成 `fact`（刀③b 之后 previous 改从文件取，这一档自动消失）。
    pub(super) fn sync_write(
        ws: &str,
        key: &str,
        value: &str,
        tags: &str,
        ty: Option<&str>,
        at: i64,
    ) -> Option<String> {
        if !memlayout::files_enabled() {
            return None;
        }
        let Ok(root) = memlayout::memory_root() else {
            return Some("md sync failed: data root unavailable".into());
        };
        let kept = match ty {
            Some(t) => t.to_string(),
            None => memlayout::read_fact(&root, ws, key)
                .ok()
                .flatten()
                .map(|f| f.ty)
                .unwrap_or_default(),
        };
        sync(|| {
            memlayout::write_fact(
                &root,
                &FactEntry {
                    workspace: ws,
                    key,
                    value,
                    tags,
                    source: "explicit",
                    updated_at: at,
                    // 四类型里唯一由模型报的一维（工具参数 `type`）；空 / 值域外由
                    // `normalize_type` 收成未分类，落盘仍是旧形状。
                    ty: &kept,
                },
            )
            .map(|_| ())
        })
    }

    pub(super) fn sync_delete(ws: &str, key: &str) -> Option<String> {
        if !memlayout::files_enabled() {
            return None;
        }
        let Ok(root) = memlayout::memory_root() else {
            return Some("md sync failed: data root unavailable".into());
        };
        sync(|| memlayout::delete_fact(&root, ws, key).map(|_| ()))
    }
}
