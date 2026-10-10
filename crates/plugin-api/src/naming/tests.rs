//! 落盘标识符规则单测（M-23 收敛：规则只有一份，这里钉死它的边界）。

use super::{SESSION_ID_MAX, valid_session_id};

#[test]
fn real_id_shapes_are_accepted() {
    // 桌面缺省形状 + 子 Agent 派生形状 + 带点的形状（分叉前 memory-tree
    // 会拒、session-store 会收 = 树投影静默失效的那一类，现在两侧同判）
    assert!(valid_session_id("desktop-1791543571572"));
    assert!(valid_session_id("desktop-1791543571572-sub-3"));
    assert!(valid_session_id("s1"));
    assert!(valid_session_id("a.b_c-1"));
    assert!(
        valid_session_id(&"x".repeat(SESSION_ID_MAX)),
        "上限本身合法"
    );
}

#[test]
fn traversal_and_noise_shapes_are_rejected() {
    for bad in [
        "",
        ".",
        "..",
        "...",
        "a/b",
        "a\\b",
        "a:b",
        "a b",
        "会话",
        "a\0b",
        &"x".repeat(SESSION_ID_MAX + 1),
    ] {
        assert!(!valid_session_id(bad), "{bad:?} 必须拒");
    }
}

// 「两侧同判」的回归锁落在**消费方各自**的测试里（依赖方向是
// session-store / memory-tree → plugin-api，本 crate 反向引用不到它们）：
// `session_store::persist::tests::dotted_session_id_round_trips` 与
// `memory_tree::storage::tests::dotted_session_id_round_trips`。
