//! 刀② 落盘层判据：幂等、按会话隔离、非法 id 拒绝、切点安全（specs/09 AC-S2-3 的前三格）。

use super::*;

fn store(tag: &str) -> SpillStore {
    // 参给的是**数据根**，不是 spill 目录——目录名由模块内那枚常量决定。
    SpillStore::new(&crate::testutil::unique_dir(&format!("spill-{tag}")))
}

/// 同内容两次落盘 = 一个文件，且两次返回的路径字符串相同。
/// 这条是 M1 前缀缓存判据的地基：路径相同 ⇒ 占位文本逐字节相同 ⇒ 被清前缀不打断。
#[test]
fn same_content_is_idempotent_and_stable() {
    let s = store("idempotent");
    let body = "一整段 shell 输出\n".repeat(200);
    let first = s.put("s-1", &body).unwrap();
    let second = s.put("s-1", &body).unwrap();
    assert_eq!(first, second, "同内容必须同路径");
    let dir = std::path::Path::new(&first).parent().unwrap();
    let count = std::fs::read_dir(dir)
        .map(|rd| rd.count())
        .expect("会话目录应在场");
    assert_eq!(count, 1, "第二次不许再写一份");
}

/// 落的是**全文**，不是预览：模型读回时必须拿得到尾巴（这正是改前纯截断丢掉的那部分）。
#[test]
fn file_holds_the_whole_output() {
    let s = store("full-text");
    let body = "头\n".to_string() + &"中".repeat(5_000) + "\n尾";
    let path = s.put("s-2", &body).unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), body);
}

/// 会话隔离：同一份内容在两个会话下各留一份文件（路径含各会话自己的子目录）。
/// 不按会话分的话，"删会话顺手删它的 spill"就没有可删的边界。
#[test]
fn sessions_get_their_own_subdirectory() {
    let s = store("per-session");
    let body = "同一份内容".repeat(10);
    let a = s.put("s-a", &body).unwrap();
    let b = s.put("s-b", &body).unwrap();
    assert_ne!(a, b, "两个会话不该共用一个路径");
    assert!(
        a.contains("s-a") && b.contains("s-b"),
        "路径里要看得见会话身份"
    );
}

/// 非法会话 id 一律拒绝落盘：这个字符串直接当目录名用，不校验 = 把路径穿越
/// 送到落盘通路上。判据取的是 `plugin_api::naming::valid_session_id` 那把唯一尺
/// （session-store 与 memory-tree 落盘用的都是它），所以口径与日志文件名一致。
#[test]
fn an_illegal_session_id_never_touches_the_disk() {
    let s = store("illegal");
    for bad in ["", "../escape", "a/b", "with space", ".."] {
        let err = s.put(bad, "内容").expect_err("非法 id 必须报错");
        assert!(err.contains("非法会话 id"), "文案要点名是 id 的问题: {err}");
    }
    // 拒绝路径不许留下任何目录（校验排在 create_dir_all 之前，那是这条断言的靶）：
    // 一次合法落盘之后，spill 那一层里只应看得见那一个会话子目录。
    let ok = s.put("s-ok", "内容").unwrap();
    let session_dir = std::path::Path::new(&ok).parent().unwrap();
    let spill_root = session_dir.parent().unwrap();
    let names: Vec<String> = std::fs::read_dir(spill_root)
        .expect("spill 层应在场")
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        names,
        vec!["s-ok".to_string()],
        "非法 id 不该留下目录: {names:?}"
    );
}

/// 切点必须落在字符边界上：中文按字节硬切会当场 panic，这不是假设中的风险
/// （`fs_read` 读一个中文文件，边界落在多字节序列中间就会撞上）。
#[test]
fn the_cut_never_splits_a_multibyte_char() {
    let body = "一二三四五"; // 每字 3 字节，总 15 字节
    for limit in [0, 1, 3, 4, 7, 15, 99] {
        let cut = char_boundary_cut(body, limit);
        assert!(
            body.is_char_boundary(cut),
            "limit={limit} 切点 {cut} 不在边界"
        );
        assert!(cut <= limit.min(body.len()), "切点不许超过请求的预算");
        // 不 panic 就是这条判据的全部；顺手把切片也做一次，字符串切片同样会 panic。
        let _ = &body[..cut];
    }
    assert_eq!(char_boundary_cut(body, 99), 15, "预算大于内容时切在末尾");
}

/// 占位文本的形状：路径、原字节数、预览字节数、读回办法四件都必须在场，
/// 少一件模型就只能猜。模板本身只许这一处（判据 SS17a 数的是文件集合）。
#[test]
fn the_placeholder_carries_path_sizes_and_a_way_back() {
    let text = persisted("F:/root/spill/s-1/deadbeef.txt", "前段预览", 12, 4_096);
    for needle in [
        "<persisted-output",
        "path=\"F:/root/spill/s-1/deadbeef.txt\"",
        "total_bytes=\"4096\"",
        "preview_bytes=\"12\"",
        "前段预览",
        "fs_read",
        "</persisted-output>",
    ] {
        assert!(text.contains(needle), "占位文本缺了 {needle}:\n{text}");
    }
}

/// 降级文案 = 改前那句截断提示（逐字），等价性另一半在 `llm/tests/tool_budget.rs`。
#[test]
fn the_degraded_text_is_the_old_truncation_notice() {
    let body = "x".repeat(100);
    let text = clipped(&body, 40);
    assert!(text.starts_with(&"x".repeat(40)));
    assert!(
        text.contains("原输出 100 字节，此处只有前 40 字节"),
        "{text}"
    );
    assert!(!text.contains("persisted-output"), "没落盘就不许声称落了盘");
}
