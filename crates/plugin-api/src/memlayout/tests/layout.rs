//! 寻址面（自 `tests.rs` 搬入，用例本体逐字未改）：项目目录名的可读性与防冲突 /
//! 未归属拒答 / 恶意 workspace 串关在 root 之内 / 键位文件名的安全化 / 总开关语义。

use super::*;

// 两侧都跑（2026-10-17 撤掉曾经的 `#[cfg(windows)]`）：这条要的是"目录名可读 + 同名
// 不同路径不冲突"，跟平台怎么解析路径无关——只要 `project_dir_name` 取末段时
// 把 `\` 与 `/` 同等当边界（实现里写了为什么这不是搬 Windows 语义）。
#[test]
fn project_dir_name_is_readable_and_collision_safe() {
    let a = project_dir_name(r"F:\Warehouse\AIwork\Locowrk");
    assert!(a.starts_with("Locowrk-"), "可读前缀取路径末段：{a}");
    assert_eq!(a.len(), "Locowrk-".len() + 8, "短哈希 = 8 hex");
    assert_eq!(
        a,
        project_dir_name(r"F:\Warehouse\AIwork\Locowrk"),
        "同路径恒等"
    );
    assert_ne!(
        project_dir_name(r"D:\code\Locowrk"),
        a,
        "同名不同路径不冲突（哈希区分）"
    );
    assert!(
        project_dir_name("").starts_with("project-"),
        "空路径兜底可读名"
    );
}

/// 队列 Q15：「这个项目的人类层家目录在哪」全仓只许一处算法，且它对未归属
/// **拒答**。断言里的期望路径全部手抄（不叫 `project_dir` 自己给自己当参照），
/// 变异 M1（把 `''` 那半撤掉）打的正是这里。
#[test]
fn project_dir_rejects_unowned_and_is_the_single_formula() {
    let root = std::path::Path::new(r"X:\data\memory");
    assert!(
        project_dir(root, "").is_err(),
        "未归属不是项目：家目录必须拒答，而不是给一个伪项目目录（那是级联删除能吃下全局键位的入口）"
    );
    let ws = r"F:\Warehouse\AIwork\a-proj";
    let dir = project_dir(root, ws).expect("项目支有落点");
    assert_eq!(
        dir,
        root.join("projects").join(project_dir_name(ws)),
        "绝对落点 = projects/<可读名-短哈希>"
    );
    // 相对支（写事实用的那个）与绝对支必须同出一处：拼上 root 后逐段相等
    assert_eq!(
        root.join(fact_rel_path(ws, "k1")),
        dir.join("facts").join("k1.md"),
        "事实文件 = 家目录/facts/<stem>.md，两份投影同源"
    );
    assert_eq!(
        unaffiliated_dir(root),
        root.join("projects").join("_unaffiliated"),
        "未归属的 tree.md 之家固定 _unaffiliated（走 PROJECTS_DIR 一处字面量）"
    );
    assert_ne!(
        unaffiliated_dir(root),
        dir,
        "未归属不得被并成伪项目目录（并了 = 改磁盘形状）"
    );
}

/// 恶意 / 畸形 workspace 串只能落进 root 之内：目录名是「安全化 + fnv 短哈希」
/// 的产物，`..` 与分隔符都进不了那一个名字。新命令收的是 workspace 而不是路径，
/// 所以这条性质就是它的防线本身（比 `reveal_in_explorer` 的 fsRoots 白名单更窄）。
// 两侧都跑（2026-10-17 撤掉曾经的 `#[cfg(windows)]`）：这条断言的是**安全性质**，
// 而 Windows 的出题法把两种分隔符形态混在一起，正好能在 POSIX 上验同一件事——
// 恶意串里既有 `\` 也有 `/`，目录名都不许带上穿越材料。
#[test]
fn project_dir_keeps_any_workspace_string_under_the_root() {
    let root = std::path::Path::new(r"X:\data\memory");
    for evil in [
        "../../etc",
        r"C:\Windows\..",
        "..",
        r"..\..\windows",
        " ",
        "F:/a/b",
        "a\\b\\..\\c",
        // POSIX 形态：同一件事在 `/` 侧的写法（撤掉平台标记时补的，不是新语义）
        "/etc/passwd",
        "/../../..",
        "//..//..//",
        "/",
        "....",
        r"a\..\..\b",
    ] {
        let dir = project_dir(root, evil).unwrap_or_else(|e| panic!("{evil:?} 应有家目录: {e}"));
        assert!(dir.starts_with(root), "{evil:?} 的落点跑出 root: {dir:?}");
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        assert!(
            !name.contains('/') && !name.contains('\\') && !name.contains(".."),
            "{evil:?} 的目录名含穿越材料: {name}"
        );
    }
}

#[test]
fn key_file_stem_is_filename_safe_and_reserved_aware() {
    assert_eq!(key_file_stem("user.language"), "user.language", "点号保留");
    assert_eq!(key_file_stem(r"a/b\c:d"), "a_b_c_d", "路径字符换下划线");
    assert_eq!(key_file_stem("con"), "_con", "Windows 保留名前缀");
    assert_eq!(key_file_stem("com10.md"), "com10.md", "com 后非 1–9 不保留");
    assert_eq!(key_file_stem("..."), "_", "全点剥成占位");
}

#[test]
fn enabled_from_env_semantics() {
    assert!(enabled_from(None), "env 未设 = 开");
    assert!(enabled_from(Some("1")));
    assert!(!enabled_from(Some("0")), "仅 \"0\" = 关");
}
