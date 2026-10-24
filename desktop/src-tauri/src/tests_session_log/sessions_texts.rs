//! T28 夹具：会话命令三组「同一事实两个出处」并处后的**文案钉子**。
//! 用户看到的句子一个字不许漂（specs/03 T28：先有用例再动形状，不许顺手统一文案）。
//! 逐字锁住两组纯逻辑——忙守卫三句（删 / 编辑重发 / 裁决，尾巴刻意不同：
//! 删除有开发者开关可强删，另两个只能等这轮结束）与 fsRoots 拒答两句
//! （「不能关联」/「拒绝打开」，动词随动作）。改这些文案 = 必须先改这里
//! （用户可见的语义改动，单独说明）；`shell_open` 的两条失败标签
//! （「打开文件管理器失败」/「打开链接失败」）在 spawn 面测不进，靠调用点
//! 字面量不改 + 真窗口目检兜底。

// import 走 glob 链出口 `crate::commands::`（`mod sessions` 是私有的；与
// `export_title.rs` 同一通道——glob 按「较小可见性」把 pub(crate) 项逐级带出，T29）。
use crate::commands::{busy_guard_msg, ensure_in_fs_roots, BUSY_TAIL_DELETE, BUSY_TAIL_WAIT_TURN};

#[test]
fn busy_guard_texts_are_byte_exact() {
    assert_eq!(
        busy_guard_msg("删除", BUSY_TAIL_DELETE),
        "会话正在运行，暂不能删除（稍后再试，或在 设置 → 开发者 允许强制删除）"
    );
    assert_eq!(
        busy_guard_msg("编辑重发", BUSY_TAIL_WAIT_TURN),
        "会话正在运行，暂不能编辑重发（等这轮结束再试）"
    );
    assert_eq!(
        busy_guard_msg("裁决", BUSY_TAIL_WAIT_TURN),
        "会话正在运行，暂不能裁决（等这轮结束再试）"
    );
}

#[test]
fn fs_roots_denial_texts_are_byte_exact() {
    let roots = vec!["F:/w".to_string()];
    assert_eq!(
        ensure_in_fs_roots(&roots, "F:/x", "不能关联"),
        Err("「F:/x」不在 设置 → 工具 → 工作区目录 里，不能关联".to_string())
    );
    assert_eq!(
        ensure_in_fs_roots(&roots, "F:/x", "拒绝打开"),
        Err("「F:/x」不在 设置 → 工具 → 工作区目录 里，拒绝打开".to_string())
    );
    assert_eq!(ensure_in_fs_roots(&roots, "F:/w", "不能关联"), Ok(()));
}
