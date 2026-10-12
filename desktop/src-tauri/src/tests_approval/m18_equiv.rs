//! M-18e② 等价证明：`approval::is_outside` 与 HEAD 原文逐格相等（批 F-24a）。
//!
//! 改后生产版把 `!t.starts_with(&format!("{r}\\"))` 换成
//! `t.strip_prefix(&r).is_none_or(|rest| !rest.starts_with('\\'))`——省掉
//! 「每个根每次判定都新建一个 String」的堆分配。这类改写最容易丢的是**分量
//! 边界**（`f:\ab` 不算 `f:\a` 内）与**空前缀/空根**两个退化档，所以矩阵
//! 专为它们铺格子，而不是随便挑几个路径。

use super::*;

use crate::approval::{is_outside, scan_roots};
use std::path::Path;

/// HEAD 原文（`git show HEAD:desktop/src-tauri/src/approval.rs` 的
/// `is_outside`），逐字抄、只改函数名。别当第二套实现维护，也别删——它是
/// 「零行为变化」这句话的对照面。
fn ref_is_outside_old(token: &str, roots: &[PathBuf]) -> bool {
    let norm = |s: &str| -> String {
        let s = s.replace('/', "\\");
        let s = s.strip_prefix("\\\\?\\").unwrap_or(&s);
        s.trim_end_matches('\\').to_ascii_lowercase()
    };
    let t = norm(token);
    roots.iter().all(|r| {
        let r = norm(&r.to_string_lossy());
        t != r && !t.starts_with(&format!("{r}\\"))
    })
}

/// 等值矩阵：token × 根集合逐格对账。
#[test]
fn is_outside_matches_format_prefix_reference() {
    let pb = |s: &str| PathBuf::from(s);
    let root_sets: Vec<(&str, Vec<PathBuf>)> = vec![
        ("空根集", vec![]),
        ("单根", vec![pb(r"F:\ws\a")]),
        ("单根带尾分隔", vec![pb("F:\\ws\\a\\")]),
        (
            "大小写混写双根",
            vec![pb(r"F:\WS\A"), pb(r"C:\Users\x\.Locowrk")],
        ),
        ("父目录根", vec![pb(r"F:\ws")]),
        ("空串根", vec![pb("")]),
        ("UNC 根", vec![pb(r"\\srv\share")]),
        ("中文根", vec![pb("F:\\数据")]),
        ("正斜杠根", vec![pb("F:/ws/a")]),
        ("verbatim 根", vec![pb(r"\\?\F:\ws\a")]),
    ];
    let tokens: &[&str] = &[
        "",
        r"F:\ws\a\x.txt",
        "F:/ws/a/x.txt",
        r"F:\WS\A\sub\y",
        r"F:\ws\ab",
        r"F:\ws\a",
        r"f:\ws\a\",
        r"C:\Windows\system.ini",
        r"\\srv\share\f",
        r"\\srv\sharex\f",
        r"\\?\F:\ws\a\x",
        "F:\\数据\\文件.txt",
        "F:\\数据x\\文件.txt",
        r"F:\ws",
        r"\ws\a",
    ];
    let (mut n, mut outside, mut inside) = (0usize, 0usize, 0usize);
    for (tag, roots) in &root_sets {
        for t in tokens {
            let got = is_outside(t, roots);
            let want = ref_is_outside_old(t, roots);
            assert_eq!(got, want, "根集「{tag}」 token={t:?}");
            n += 1;
            outside += got as usize;
            inside += (!got) as usize;
        }
    }
    assert_eq!(n, root_sets.len() * tokens.len(), "矩阵必须真的跑满");
    // 阳性对照：两色都得有（全 true / 全 false 都说明格子铺错了 = 零证明力）
    assert!(
        outside > 0 && inside > 0,
        "矩阵退化：outside {outside} / inside {inside}，两者都必须非零"
    );
    // 分量边界单独钉一条：这一档是 `strip_prefix` 改写**唯一**可能悄悄放宽
    // 的地方（少了分隔符那一位就会把 `f:\ws\ab` 判成 `f:\ws\a` 内 = 越界门
    // 漏报，安全面退化）。既有测试 `scanner_extracts_and_judges_paths` 也
    // 钉了同一档，这里再钉一次是因为它现在同时被参考实现对账覆盖。
    let roots = vec![pb(r"F:\ws\a")];
    assert!(!is_outside(r"F:\ws\a\x", &roots), "根内 = 不越界");
    assert!(
        is_outside(r"F:\ws\ab", &roots),
        "分量边界：兄弟目录不算根内"
    );
    assert!(!is_outside(r"F:\ws\a", &roots), "与根相等 = 不越界");
}

/// `scan_roots` 真的采纳**传进来的** ctx 围栏：M-18a 把「自己拿 resolver +
/// session_id 去解析」换成「收已解析的 `CallContext`」，这条钉住换完没把围栏
/// 弄丢。空围栏 / 无 ctx = 回落 env 全局（既有语义，不许因签名改了走样）。
///
/// 为什么要有这条正向测试而不能只靠变异：把 ctx 围栏弄丢的那类变异会让既有
/// `external_path_forces_card_even_in_auto` 的第一条断言落进决策卡，而那条
/// 测试的 hub 缺省超时曾是 `Duration::ZERO`（= 不起守夜任务）、决断者又在
/// 断言之后才 spawn ⇒ 它会**挂起**而不是失败（批 F-24a 施工时实测确认；已
/// 同批把那条测试的超时改成 500ms 让它快速失败，见该处注释）。
#[test]
fn scan_roots_uses_the_passed_context_fence() {
    let fence = "F:\\lw-scan-root";
    let ctx = CallContext {
        session_id: Some("s1".into()),
        workspace: None,
        fs_roots: vec![fence.into()],
    };
    let roots = scan_roots(Some(&ctx));
    assert!(
        roots.iter().any(|r| r == Path::new(fence)),
        "ctx 围栏必须进扫描根：{roots:?}"
    );
    // 空围栏 / 无 ctx = 回落 env（不得凭空出现围栏根）
    let empty = CallContext {
        session_id: Some("s1".into()),
        workspace: None,
        fs_roots: vec![],
    };
    for (tag, got) in [
        ("空围栏", scan_roots(Some(&empty))),
        ("无 ctx", scan_roots(None)),
    ] {
        assert!(
            !got.iter().any(|r| r == Path::new(fence)),
            "{tag} = 回落 env，不得凭空出现围栏根：{got:?}"
        );
    }
}
