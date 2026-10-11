//! 截断等值回归（批 F·F2）：新函数必须与**收敛前那 6 份本地实现逐字节相等**。
//!
//! 参考实现不是手写的期望值，而是**从被删的本地 `clip` 原文抄进来**的（文件与函数
//! 名标在注释里）。这样「等值」是机器判的：只要新函数与旧实现行为有任何一点不同
//! ——包括省略号加不加、trim 在哪一层、换行折不折——断言当场红。

use super::{clip_ellipsis, clip_markdown, clip_plain, clip_trimmed};

// ---- 收敛前的 6 份本地实现（原文照抄，只改名）----

/// browser/console.rs:46（TEXT_CLIP 固定宽度）
fn old_plain(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// notifications.rs:80
fn old_notif_clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// organize/search_titles.rs:235
fn old_title_clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect::<String>() + "…"
    }
}

/// subagent_verify.rs:130
fn old_verify_clip(s: &str, n: usize) -> String {
    let mut out: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        out.push('…');
    }
    out
}

/// scheduler/tools.rs:296
fn old_sched_clip(s: &str, max_chars: usize) -> String {
    s.trim().chars().take(max_chars).collect()
}

/// export.rs:12（CLIP = 400，原实现里常量在此参数化）
fn old_export_clip(s: &str, n: usize) -> String {
    let mut out: String = s.chars().take(n).collect();
    if s.chars().count() > n {
        out.push_str("…（截断）");
    }
    out.replace('\n', " ")
}

/// 覆盖到四种语义的分岔面：空串 / 恰好 n / n+1 / 远超 n / CJK / 混合 /
/// 换行在截断窗口内外 / 前后空白 / 省略号本身参与计数。
fn corpus() -> Vec<String> {
    let mut v: Vec<String> = vec![
        String::new(),
        "a".into(),
        "abc".into(),
        "hello".into(),
        "hello world".into(),
        "   padded   ".into(),
        "line1\nline2\nline3".into(),
        "short\nline".into(),
        "…already ellipsized…".into(),
        "中文".into(),
        "跨会话取回闭环已完成".into(),
        "混合 mixed 文本 text 混 mixed 排".into(),
        "尾随换行\n".into(),
        "\n\n全空白\n\n".into(),
        "émoji 与组合字符 😀 计数".into(),
    ];
    // 造一批正好落在 n 边界两侧的长度
    for len in [
        0usize, 1, 2, 3, 30, 31, 39, 40, 41, 79, 80, 81, 119, 120, 121,
    ] {
        v.push("字".repeat(len));
        v.push("x".repeat(len));
    }
    v
}

#[test]
fn every_width_boundary_matches_the_old_implementations() {
    // 三档宽度覆盖各处现役常量（TEXT_CLIP 2000 / CLIP 400 / 80 / 120 / 40 / 100…）
    let c = corpus();
    for n in [0usize, 1, 2, 3, 30, 40, 80, 100, 120, 200, 400, 2000] {
        for s in &c {
            assert_eq!(clip_plain(s, n), old_plain(s, n), "plain n={n} src={s:?}");
            assert_eq!(
                clip_plain(s, n),
                old_notif_clip(s, n),
                "notif n={n} src={s:?}"
            );
            assert_eq!(
                clip_ellipsis(s, n),
                old_title_clip(s, n),
                "ellipsis(title) n={n} src={s:?}"
            );
            assert_eq!(
                clip_ellipsis(s, n),
                old_verify_clip(s, n),
                "ellipsis(verify) n={n} src={s:?}"
            );
            assert_eq!(
                clip_trimmed(s, n),
                old_sched_clip(s, n),
                "trimmed n={n} src={s:?}"
            );
            assert_eq!(
                clip_markdown(s, n),
                old_export_clip(s, n),
                "markdown n={n} src={s:?}"
            );
        }
    }
}

#[test]
fn semantics_actually_differ_so_names_are_not_cosmetic() {
    // 四条语义必须真不同——若哪天被"顺手统一"成一条，这里会红：
    // 同一条输入在四种形态下给出四种可区分的结果。
    let s = "  第一行\n第二行内容很长需要截断  ";
    let n = 6;
    let plain = clip_plain(s, n);
    let ell = clip_ellipsis(s, n);
    let trim = clip_trimmed(s, n);
    let md = clip_markdown(s, n);
    assert_eq!(plain, "  第一行\n"); // 不 trim、不加标记、保留换行
    assert_eq!(ell, "  第一行\n…"); // 同上 + 省略号
    assert_eq!(trim, "第一行\n第二"); // 先 trim，无标记
    assert_eq!(md, "  第一行 …（截断）"); // 不 trim，但换行折平 + 长标记
    assert_ne!(plain, ell);
    assert_ne!(plain, trim);
    assert_ne!(ell, md);
}

#[test]
fn markdown_export_shape_is_pinned() {
    // 导出件的用户可见金样（逐字节钉死，不是"包含"级断言）：
    // 短文本原样、含换行折平、超长带 `…（截断）`。
    assert_eq!(clip_markdown("一行简短输出", 400), "一行简短输出");
    assert_eq!(
        clip_markdown("a\nb\nc", 400),
        "a b c",
        "换行必须折成空格，否则引用行被打断"
    );
    let long = "字".repeat(401);
    let out = clip_markdown(&long, 400);
    // 400 个字符 + 标记 `…（截断）`（它本身是 5 个 char：… （ 截 断 ））
    assert_eq!(out.chars().count(), 405);
    assert!(out.ends_with("…（截断）"));
    assert!(!out.contains('\n'));
}

#[test]
fn cjk_never_splits_inside_a_character() {
    // 按 char 计数：字节切片写法（&s[..n]）在这里会 panic。
    for n in 1..8 {
        let out = clip_plain("中文混mixed", n);
        assert!(out.chars().count() <= n);
        assert!(!out.is_empty());
    }
}
