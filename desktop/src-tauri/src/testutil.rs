//! 测试临时目录唯一入口（《AUDIT-FIX-PLAN-v1》批 F·F11，**审计完全没覆盖**的一类：
//! 它查死代码与重复实现，「测试脚手架脏」只有真跑门禁才现形）。
//!
//! 收敛前 desktop 测试里有 **40+ 处**各写一遍的取目录代码，形态分三档：
//! ①`{前缀}-{tag}-{pid}` **且不先清**——最危险的一档：Windows 会**回收复用 pid**，
//! 于是本次运行读到上一次运行留下的残目录，断言数的是"这次追加了几条"就直接对不上
//! （批 B 实测撞红过一次：`.undo.bak` 读出 6 条 vs 期望 4，满跑红单跑绿）；
//! ②`{前缀}-{tag}-{pid}` + 先清——不脏读，但同进程内两个测试用同 tag 会互踩；
//! ③带纳秒 nonce——不脏读也不互踩，但目录名每次不同 → **盘上只增不清**。
//!
//! 现在统一 = **pid + 纳秒 + 进程内自增序号**三重唯一（同纳秒连续两次调用也不撞），
//! 取到即先清，两个入口按「调用方要不要目录已经存在」分：
//!
//! * [`work_dir`] —— 已建好、保证是空的空目录（测试往里写落盘产物）
//! * [`unique_dir`] —— 只给唯一路径，**故意不创建**（测「路径不存在」的分支：
//!   catalog 缺席回落、fail-closed 越界拒绝、`SettingsStore` 缺文件出默认值）
//!
//! **本入口治的是「脏读」，不治「累积」**：带 nonce 的目录每次都不一样，跑完留在
//! `%TEMP%` 里没人删——要收这条得给每个调用点挂 Drop 守卫（40+ 处路径会被 move 进
//! store / 闭包，逐点判生命周期），与 F11 的病（pid 复用读到上次残留）不是一回事，
//! 已单独登记待裁定，不在本批扩大改动面。
//!
//! 名字里的 nonce 属于「发号」不是「读时间」，所以**不引 `crate::timeutil`**（那把尺
//! 的头注明写纳秒造号故意不收），这里自带一行取 `as_nanos()`。

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

/// 进程内自增：同纳秒内的连续两次调用也必须拿到不同目录（`as_nanos()` 在
/// Windows 上粒度可达 100ns，单靠它不足以保证唯一）。
static SEQ: AtomicU64 = AtomicU64::new(0);

/// 目录名的构造规则（纯函数，`nanos`/`seq` 由调用方给 = 这一层可测：见
/// `testutil/tests.rs` 的「同一纳秒靠 seq 分家」用例）。
/// 形状 `locowrk-{tag}-{pid}-{nanos}-{seq}`：tag 留着是为了失败时人能一眼看出
/// 这堆临时目录归哪个模块。
fn dir_name(tag: &str, nanos: u128, seq: u64) -> String {
    format!("locowrk-{tag}-{}-{nanos}-{seq}", std::process::id())
}

/// 唯一目录路径（不碰文件系统，见 [`unique_dir`]）。
fn unique(tag: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(dir_name(tag, nanos, n))
}

/// 清掉已存在内容后建空目录（与 [`unique`] 分开，是为了让"拿到手一定是空的"
/// 这条契约可测——见 `testutil/tests.rs` 里预存脏文件的用例）。
fn prepared(d: PathBuf) -> PathBuf {
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("testutil::prepared: 临时目录创建失败");
    d
}

/// 唯一且**已建好的空目录**（等价于收敛前的「pid + 先清 + create_dir_all」形态）。
pub(crate) fn work_dir(tag: &str) -> PathBuf {
    prepared(unique(tag))
}

/// 唯一且**故意不存在**的路径（父目录也不预建）。给「读不到 / 目录缺席 / fail-closed」
/// 这类分支用——收敛前那些写死名字的 `…-catalog-absent` / `…-missing-dir` 属于本档。
pub(crate) fn unique_dir(tag: &str) -> PathBuf {
    unique(tag)
}

#[cfg(test)]
mod tests;
