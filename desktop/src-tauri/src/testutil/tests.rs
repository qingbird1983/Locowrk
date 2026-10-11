//! 临时目录入口的行为锁（批 F·F11）。
//!
//! 这里锁的是**契约**而不是某次运行的巧合：唯一性、拿到即空、`unique_dir` 绝不落盘。
//! 「唯一 + 先清」正是收敛前那一档 `{前缀}-{tag}-{pid}` 且不先清的写法所缺的东西
//! ——Windows 复用 pid 时，缺了它就会把上一次运行的残留当成本次数据（批 B 实测撞红）。

use super::*;
use std::fs;

#[test]
fn every_call_gets_its_own_empty_directory() {
    // 200 次同 tag：路径必须两两不同，且每个都存在、都是空的。
    let mut seen = std::collections::HashSet::new();
    for _ in 0..200 {
        let d = work_dir("uniq");
        assert!(
            seen.insert(d.clone()),
            "同 tag 连续调用拿到了同一个目录：{d:?}"
        );
        assert!(d.is_dir(), "work_dir 必须已经把目录建好：{d:?}");
        assert_eq!(
            fs::read_dir(&d).map(|r| r.count()).unwrap_or(usize::MAX),
            0,
            "work_dir 交出来的必须是空目录：{d:?}"
        );
    }
}

#[test]
fn a_taken_name_is_cleared_before_it_is_handed_back() {
    // 「拿到即空」在名字被占用的那一条分支上也要成立：先用 unique_dir 造一个带脏
    // 文件的目录，再走 prepared —— 旧内容必须被清掉。
    let d = unique_dir("dirty");
    fs::create_dir_all(d.join("sub")).unwrap();
    fs::write(d.join("a.txt"), "上一次运行的残留").unwrap();
    fs::write(d.join("sub/b.bin"), "junk").unwrap();
    assert!(d.join("a.txt").exists());

    let clean = prepared(d.clone());
    assert!(clean.is_dir());
    assert_eq!(fs::read_dir(&clean).map(|r| r.count()).unwrap_or(9), 0);
    assert!(
        !clean.join("a.txt").exists(),
        "残留没清 = 本次测试会读到上次的数据"
    );
    let _ = fs::remove_dir_all(&clean);
}

#[test]
fn unique_dir_never_touches_the_filesystem() {
    // 「目录缺席」类用例靠的就是这个：取路径不产生任何盘上副作用。
    let d = unique_dir("absent-branch");
    assert!(!d.exists(), "unique_dir 不许创建目录：{d:?}");
    assert!(!d.parent().unwrap().join("absent-branch-marker").exists());
}

#[test]
fn same_nanosecond_is_split_by_seq() {
    // 唯一性的第二道保险：时钟给不出新纳秒时（Windows 粒度可达 100ns，快速循环
    // 完全可能撞），靠进程内自增 seq 分家。少了这一维，同纳秒的两次调用会拿到
    // 同一个目录——两个并行测试互删对方的落盘产物，表现为"满跑红单跑绿"。
    let a = dir_name("seq", 1_700_000_000_000_000_000, 0);
    let b = dir_name("seq", 1_700_000_000_000_000_000, 1);
    assert_ne!(a, b, "同纳秒必须靠 seq 分开：{a}");
    assert_eq!(
        dir_name("seq", 1_700_000_000_000_000_000, 0),
        a,
        "纯函数同入参同输出"
    );
}

#[test]
fn the_name_carries_tag_and_pid_so_a_human_can_trace_it() {
    let d = work_dir("traceable");
    let name = d.file_name().unwrap().to_string_lossy().to_string();
    assert!(
        name.starts_with("locowrk-traceable-"),
        "前缀/ tag 形状变了：{name}"
    );
    assert!(name.contains(&format!("-{}-", std::process::id())));
    assert_eq!(d.parent().unwrap(), std::env::temp_dir());
    let _ = fs::remove_dir_all(&d);
}
