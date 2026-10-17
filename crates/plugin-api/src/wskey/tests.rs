//! `wskey::normalize` 判据（MR §6 · 队列 Q2）。
//!
//! 主轴是**同项目单键位**：同一个真实目录的多种写法必须落成一个串。夹具用
//! `env!("CARGO_MANIFEST_DIR")`（**本机一定存在**的目录）而不是编一个字符串去
//! 迎合实现——Q1b 立的口径「夹具必须照结果构造代码抄」在这里的同族形态。
//!
//! 分两档写：**纯函数档**（`win_form` / `posix_form` / `strip_verbatim`）跨平台
//! 都跑，于是 Linux CI 也能验 Windows 的收尾逻辑；**`normalize` 档**按 `cfg!`
//! 分开钉，因为"本平台该折叠/不该折叠"是真实语义，Linux 上断言小写折叠会
//! 判出一条假绿。

use std::path::Path;

use super::*;

/// 本机确实存在的一个目录（当前 crate 的源码目录）。
fn real_dir() -> String {
    env!("CARGO_MANIFEST_DIR").to_string()
}

/// 必然不存在的目录（带 pid 防并发撞名）。
fn missing_dir() -> String {
    format!(
        "{}/q2-no-such-workspace-{}",
        env!("CARGO_MANIFEST_DIR"),
        std::process::id()
    )
}

// ------------------------------------------------------------ 未归属键位

#[test]
fn empty_and_blank_both_land_on_the_unattributed_keyside() {
    // `""` = 全局偏好层（`workspace` 列 NOT NULL DEFAULT ''）；带空白的串不许
    // 原样进主键（那是"看起来像另一个项目"的第三种写法）。
    assert_eq!(normalize(""), "");
    assert_eq!(normalize("   "), "");
    assert_eq!(normalize("\t \n"), "");
}

// ------------------------------------------------------------ 同项目单键位

#[test]
fn an_existing_directory_keeps_pointing_at_itself() {
    // 后面所有合并断言的参照系：先确立"存在的路径归一后仍指得回那个目录"。
    let d = real_dir();
    let k = normalize(&d);
    assert!(!k.is_empty(), "存在的目录不许归成空串（那是未归属）");
    assert!(
        Path::new(&k).exists(),
        "归一键位必须还指得回那个目录: {k} ← {d}"
    );
}

#[test]
fn trailing_slash_and_dot_never_split_a_keyside() {
    // 这两类漂移**跨平台成立**（不涉及大小写、不涉及反斜杠语义）。
    let d = real_dir();
    let base = normalize(&d);
    for variant in [format!("{d}/"), format!("{d}///"), format!("{d}/.")] {
        assert_eq!(normalize(&variant), base, "写法 {variant:?} 不该造出新键位");
    }
}

// 两侧都跑（2026-10-17 撤掉曾经的 `#[cfg(windows)]`）：这条要的性质是"同一个目录的两种
// 写法必须落成一个键位"，两侧都需要。Windows 的路径 API 对**不存在的中间段**也会折叠 `..`，
// POSIX 的 realpath 遇缺失段直接报错 ⇒ 靠 `lexical_fold` 在回落分支里补上这一步，
// 于是 `<parent>/x/../<name>` 与 `<parent>/<name>` 在两个平台上同键位。
#[test]
fn dotdot_inside_the_path_is_resolved_not_left_as_text() {
    // `F:\proj\a\..\b` 与 `F:\proj\b` 是同一个目录。词法上"看着在一处"而 OS
    // 解析后另指一处，是键位与围栏共同的坑（`fsutil::resolve_for_write` 为
    // 同一件事写过一条注释）。
    let dir_s = real_dir();
    let dir = Path::new(&dir_s);
    let parent = dir.parent().expect("crate dir has a parent");
    let name = dir
        .file_name()
        .and_then(|s| s.to_str())
        .expect("utf-8 crate dir name");
    let via_dotdot = parent.join("q2-not-there").join("..").join(name);
    assert_eq!(
        normalize(&via_dotdot.to_string_lossy()),
        normalize(&dir_s),
        "`<parent>/x/../<name>` 必须与 `<parent>/<name>` 同键位"
    );
}

#[test]
fn a_missing_path_falls_back_instead_of_failing_or_going_global() {
    // 项目目录被删/还没建：归一**不许报错**（记忆读写照旧），也**不许返回空串**
    // （空串 = 全局偏好层，会把"某个不存在的项目"的事实污染成用户全局偏好）。
    let m = missing_dir();
    let k = normalize(&m);
    assert!(!k.is_empty(), "不存在的路径不许落成未归属键位: {m}");
    assert!(
        !k.ends_with('/') && !k.ends_with('\\'),
        "回落分支同样剥尾分隔符: {k}"
    );
}

#[cfg(windows)]
#[test]
fn windows_collapses_case_and_slash_style_into_one_keyside() {
    let d = real_dir();
    let base = normalize(&d);
    assert_eq!(normalize(&d.to_uppercase()), base, "全大写写法必须同键位");
    assert_eq!(normalize(&d.to_lowercase()), base, "全小写写法必须同键位");
    assert_eq!(
        normalize(&d.replace('\\', "/")),
        base,
        "正斜杠写法必须同键位（旁车存反斜杠，模型常写正斜杠）"
    );
    assert_eq!(normalize(&format!(r"{d}\")), base, "尾反斜杠写法必须同键位");
    // 结果形态自身：折叠过、无 verbatim 前缀、分隔符统一
    assert_eq!(base, base.to_lowercase(), "Windows 键位恒小写");
    assert!(!base.contains(r"\\?\"), "verbatim 前缀不许进键位: {base}");
    assert!(
        !base.contains('/'),
        "Windows 键位分隔符统一为反斜杠: {base}"
    );
}

#[cfg(windows)]
#[test]
fn missing_and_existing_projects_agree_across_the_two_branches() {
    // 红利窗口的真实形态：项目从没被写过时键位走回落分支，而它必须与"目录存在"
    // 那条分支给出**同一个串形**——否则同一个项目在"建过/没建过"之间换键位。
    // 这里能严格比的是**串形**（大小写/分隔符/尾缀），因为不存在的那条没有
    // canonicalize 的真值可比。
    let parent = Path::new(&real_dir())
        .parent()
        .expect("crate dir has a parent")
        .to_string_lossy()
        .to_string();
    let tail = format!("q2-missing-{}", std::process::id());
    let as_backslash = format!(r"{parent}\{tail}");
    let key = normalize(&as_backslash);
    assert_eq!(
        normalize(&format!("{parent}/{tail}")),
        key,
        "斜杠漂移要合并"
    );
    assert_eq!(
        normalize(&as_backslash.to_uppercase()),
        key,
        "大小写漂移要合并"
    );
    assert_eq!(
        normalize(&format!(r"{as_backslash}\")),
        key,
        "尾分隔符漂移要合并"
    );
    // 与"存在的那条"同形：把已存在的父目录拿来比，两侧都该是折叠+反斜杠形
    let existing = normalize(&parent);
    assert!(
        existing.chars().all(|c| !c.is_ascii_uppercase()),
        "存在路径的键位也恒小写: {existing}"
    );
}

#[cfg(not(windows))]
#[test]
fn posix_never_folds_case_and_never_treats_backslash_as_a_separator() {
    // 反向判据（Linux CI 上跑的那半边）：POSIX 上 `/Tmp/X` 与 `/tmp/x` 是**不同**
    // 目录，折叠 = 把两个项目的记忆合进一个主键 = 数据损坏；反斜杠是合法文件名
    // 字符，当分隔符剥掉会指错地方。
    assert_ne!(normalize("/Tmp/X"), normalize("/tmp/x"));
    assert!(normalize("/tmp/MixedCase").contains("MixedCase"));
    assert!(
        normalize("/tmp/proj/keep\\me").ends_with("keep\\me"),
        "POSIX 不许剥反斜杠"
    );
}

// ------------------------------------------------------------ 纯函数档（跨平台都跑）

#[test]
fn verbatim_prefix_is_stripped_for_both_drive_and_unc_forms() {
    // Windows canonicalize 的产物；不剥就会带着 `\\?\` 进管理页与 md 目录名。
    assert_eq!(strip_verbatim(r"\\?\C:\x"), r"C:\x");
    assert_eq!(strip_verbatim(r"\\?\UNC\srv\share"), r"\\srv\share");
    assert_eq!(strip_verbatim(r"C:\x"), r"C:\x", "无前置时原样");
    assert_eq!(
        strip_verbatim(r"\\srv\share"),
        r"\\srv\share",
        "UNC 原样不动"
    );
    assert_eq!(strip_verbatim(""), "");
}

#[test]
fn win_form_unifies_separators_and_folds_case() {
    assert_eq!(win_form(r"F:\Warehouse\TITY"), r"f:\warehouse\tity");
    assert_eq!(
        win_form(r"F:/Warehouse/TITY"),
        r"f:\warehouse\tity",
        "混用分隔符"
    );
    assert_eq!(win_form(r"C:\x\\"), r"c:\x", "尾部冗余分隔符剥掉");
    assert_eq!(win_form(r"C:\"), r"c:\", "驱动器根不许剥成 `C:`（换语义）");
    assert_eq!(win_form("C:"), r"c:\", "同上：补回一个分隔符");
    assert_eq!(
        win_form(r"\\srv\share\"),
        r"\\srv\share",
        "UNC 共享名不是盘符根"
    );
    assert_eq!(win_form(r"\\"), r"\\", "整串都是分隔符 = 不剥");
    assert_eq!(
        win_form("Ünïcode Path"),
        "ünïcode path",
        "折叠走 Unicode 不是 ASCII-only"
    );
}

#[test]
fn posix_form_only_strips_trailing_slashes() {
    assert_eq!(posix_form("/a/b/"), "/a/b");
    assert_eq!(posix_form("/a/b///"), "/a/b");
    assert_eq!(posix_form("/"), "/", "根不许剥成空串（空串 = 未归属键位）");
    assert_eq!(posix_form("/Tmp/Mixed"), "/Tmp/Mixed", "大小写不动");
    assert_eq!(posix_form("/a/b"), "/a/b", "无尾缀时原样");
}

#[test]
fn drive_root_shape_is_only_a_two_char_letter_colon() {
    assert!(is_drive_root("c:"));
    assert!(is_drive_root("F:"));
    assert!(!is_drive_root("cc:"), "三字符不是盘符");
    assert!(!is_drive_root("1:"), "首位必须是字母");
    assert!(!is_drive_root("c"), "必须有冒号");
    assert!(!is_drive_root(""), "空串不进这条分支");
}

#[test]
fn lexical_fold_collapses_dot_and_dotdot_on_both_separator_styles() {
    // 纯函数档：`win` 是**显式参数**，所以同一台机器上两种分隔符形态都能跑一遍——
    // 它不是"平台分支"，而是"这套规则本来就有两份"（Q2 撤平台标记时补的判读）。
    assert_eq!(lexical_fold("/a/x/../b", false), "/a/b");
    assert_eq!(lexical_fold("/a/./b", false), "/a/b");
    assert_eq!(lexical_fold("/a/b/..", false), "/a");
    assert_eq!(
        lexical_fold("/..", false),
        "/",
        "根之上 clamp，不许弹成空串"
    );
    assert_eq!(lexical_fold("/a/../../..", false), "/");
    assert_eq!(
        lexical_fold("../a", false),
        "../a",
        "相对路径没到顶：`..` 留在串里"
    );
    assert_eq!(
        lexical_fold("a/..", false),
        ".",
        "全弹光要给 `.`，绝不给空串（空串 = 未归属键位）"
    );
    assert_eq!(lexical_fold(r"C:\a\x\..\b", true), r"C:\a\b");
    assert_eq!(lexical_fold(r"C:\a\.\b", true), r"C:\a\b");
    // 反斜杠在 POSIX 是合法文件名字符 ⇒ 不许当边界切
    assert_eq!(lexical_fold(r"/a/b..\c", false), r"/a/b..\c");
    // 三条保守边界：没有 `.`/`..` 段、连续分隔符、UNC 头 ⇒ 一律原样返回
    assert_eq!(lexical_fold("/a/b", false), "/a/b");
    assert_eq!(lexical_fold(r"\\srv\share\x", true), r"\\srv\share\x");
    assert_eq!(
        lexical_fold("//a/x/../b", false),
        "//a/x/../b",
        "连续分隔符不动"
    );
    assert_eq!(
        lexical_fold(r"\\srv\share\..\x", true),
        r"\\srv\share\..\x",
        "UNC 的共享名不许被 `..` 弹掉"
    );
}
