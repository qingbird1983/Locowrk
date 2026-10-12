//! 正文搜索面（自 `tests.rs` 批 F-23 拆出，既有用例本体逐字未改）：命中 / 摘录 /
//! 上限 / 空查询；外加批 F-23（审计 M-24d）**单遍 IO 重构的等价证明**。

use super::*;

#[test]
fn search_finds_content_with_snippet_and_respects_limit() {
    let dir = tmp_dir("search");
    for (id, msg) in [
        ("a", " discussion about rust internals "),
        ("b", "完全无关的中文内容"),
        ("c", "another RUST talk and more RUST notes"),
    ] {
        let p = session_file(&dir, id).unwrap();
        append_event(&p, &user_msg(msg)).unwrap();
    }
    let hits = search_sessions(&dir, "RUST", 10);
    assert_eq!(hits.len(), 2, "大小写不敏感，两个会话命中");
    assert!(
        hits.iter()
            .all(|h| h.snippet.to_lowercase().contains("ust"))
    );
    let c = hits.iter().find(|h| h.id == "c").unwrap();
    assert_eq!(c.match_count, 2, "两行消息各命中一次");
    let limited = search_sessions(&dir, "rust", 1);
    assert_eq!(limited.len(), 1);
    assert_eq!(search_sessions(&dir, "  ", 10).len(), 0);
    assert_eq!(search_sessions(&dir, "不存在的词", 10).len(), 0);
    std::fs::remove_dir_all(&dir).ok();
}

// ---- 批 F-23（审计 M-24d）：单遍 IO 重构与旧两遍实现的等价证明 ----
//
// 病根 = `search_sessions` 先调 `list_sessions`（把每个会话文件读一遍、解析一遍），
// 再逐个 `File::open` **重读一遍、重新解析一遍**——同一份 jsonl 在一次调用里过两次
// IO 两次 JSON 解析；`ci_fold` 的结果还被再分配一份等长 `Vec<char>` 只为做窗口比较。
// 改法 = 抽出 `list_entries`（只读目录项与 metadata，一个 jsonl 都不打开）+
// `scan_session_file`（一遍解析、visitor 回调），列表与搜索**共用同一层**（不是把旧
// 实现复制一份，那样只会多一处将来要同步的重复）；命中计数挂在回调上，正文不攒进
// 内存；`ci_fold` 的结果上直接比字符。下面把 HEAD 的两份原文抄成参考实现逐字段对账。

/// **被删原文**：HEAD 版 `list_sessions`（逐字抄，只改函数名）。
fn ref_list_sessions_old(dir: &Path) -> Vec<SessionMeta> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut metas: Vec<(std::time::SystemTime, SessionMeta)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
        .filter_map(|e| {
            let path = e.path();
            let id = path.file_stem()?.to_str()?.to_string();
            if !valid_session_id(&id) {
                return None;
            }
            let meta = e.metadata().ok()?;
            let modified = meta.modified().ok()?;
            let updated_at = modified
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_secs() as i64;
            let (events, title) = std::fs::File::open(&path)
                .map(|f| {
                    use std::io::BufRead;
                    let mut events = 0usize;
                    let mut title = None;
                    for line in std::io::BufReader::new(f).lines().map_while(Result::ok) {
                        if line.trim().is_empty() {
                            continue;
                        }
                        let Ok(ev) = serde_json::from_str::<SessionEvent>(&line) else {
                            continue;
                        };
                        events += 1;
                        if title.is_none() {
                            if let SessionEvent::UserMessage { content, .. } = ev {
                                title = Some(content.chars().take(SESSION_TITLE_CHARS).collect());
                            }
                        }
                    }
                    (events, title)
                })
                .unwrap_or((0, None));
            let sidecar = read_meta(dir, &id);
            let title = sidecar.as_ref().and_then(|m| m.title.clone()).or(title);
            let workspace = sidecar.as_ref().and_then(|m| m.workspace.clone());
            let permission_mode = sidecar.as_ref().and_then(|m| m.permission_mode.clone());
            let model = sidecar.as_ref().and_then(|m| m.model.clone());
            let model_provider = sidecar.as_ref().and_then(|m| m.model_provider.clone());
            let thinking_level = sidecar.and_then(|m| m.thinking_level);
            Some((
                modified,
                SessionMeta {
                    id,
                    updated_at,
                    events,
                    title,
                    workspace,
                    permission_mode,
                    model,
                    model_provider,
                    thinking_level,
                },
            ))
        })
        .collect();
    metas.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.id.cmp(&b.1.id)));
    metas.into_iter().map(|(_, m)| m).collect()
}

/// **被删原文**：HEAD 版 `search_sessions`（逐字抄；内部改调 [`ref_list_sessions_old`]
/// ——HEAD 那一版调的正是 HEAD 的 `list_sessions`，参考实现必须自洽于旧世界）。
fn ref_search_sessions_old(dir: &Path, query: &str, limit: usize) -> Vec<SessionSearchHit> {
    use std::io::BufRead;
    let needle: Vec<char> = query.trim().to_lowercase().chars().collect();
    if needle.is_empty() {
        return Vec::new();
    }
    let mut hits = Vec::new();
    for meta in ref_list_sessions_old(dir) {
        let Ok(path) = session_file(dir, &meta.id) else {
            continue;
        };
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let mut count = 0usize;
        let mut snippet: Option<String> = None;
        for line in std::io::BufReader::new(file).lines().map_while(Result::ok) {
            let Ok(ev) = serde_json::from_str::<SessionEvent>(&line) else {
                continue;
            };
            let text = match ev {
                SessionEvent::UserMessage { content, .. } => content,
                SessionEvent::AssistantMessage { content, .. } => content,
                _ => continue,
            };
            let folded = ci_fold(&text);
            let chars: Vec<char> = folded.iter().map(|(c, _)| *c).collect();
            // 非重叠窗口从左往右数命中；首处命中留摘录
            let mut at = 0usize;
            while at + needle.len() <= chars.len() {
                match chars[at..].windows(needle.len()).position(|w| w == needle) {
                    Some(rel) => {
                        let pos = at + rel;
                        count += 1;
                        if snippet.is_none() {
                            snippet = Some(snippet_around(&text, folded[pos].1, &needle));
                        }
                        at = pos + needle.len();
                    }
                    None => break,
                }
            }
        }
        if count > 0 {
            hits.push(SessionSearchHit {
                id: meta.id.clone(),
                title: meta.title.clone(),
                snippet: snippet.unwrap_or_default(),
                match_count: count,
                updated_at: meta.updated_at,
            });
            if hits.len() >= limit {
                break;
            }
        }
    }
    hits
}

fn assistant_msg(content: &str) -> SessionEvent {
    SessionEvent::AssistantMessage {
        content: content.into(),
        tool_calls: Vec::new(),
    }
}

/// 逐字段比对用（`SessionMeta` / `SessionSearchHit` 都只 derive `Serialize`，
/// 没有 `PartialEq`）：走 JSON 值比较，顺带把 `camelCase` 投影面一起钉住。
fn json<T: serde::Serialize>(v: &T) -> serde_json::Value {
    serde_json::to_value(v).expect("序列化用于比对")
}

#[test]
fn single_pass_list_and_search_match_two_pass_reference() {
    use std::io::Write;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // 目录名带纳秒并先清（历批 F11 教训：只带 pid 会读到上次运行残留）
    let dir = std::env::temp_dir().join(format!("locowrk-persist-m24d-{nanos}"));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();

    // s-a：命中词大小写混合 + 非重叠计数（`aaa` 里找 `aa` 只算 1 次）+ 长前缀行 +
    //      换行（摘录压成空格）
    let a = session_file(&dir, "s-a").unwrap();
    append_event(&a, &user_msg("Discussion about Rust internals")).unwrap();
    append_event(&a, &assistant_msg("rustc 的借用检查：Rust rust 反复出现")).unwrap();
    append_event(&a, &user_msg("aaa")).unwrap();
    append_event(
        &a,
        &user_msg(&format!("{}命中在很后面", "很长的前缀".repeat(12))),
    )
    .unwrap();
    append_event(&a, &user_msg("第一行\n第二行 rust")).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));

    // s-b：**不命中**的会话也带旁车标题（新实现只在真有命中时读旁车 ⇒ 这一格专门
    //      钉"少读一次旁车不改变结果"）；含 `to_lowercase` 变长字符（İ → i + 组合点）。
    //      注意：这里那条 `repeat(6)` 的长消息**不走**标题截断——标题只取首条用户
    //      消息，而它是本会话第三条（首跑把截断路径记在这一格，阳性对照当场判红）。
    let b = session_file(&dir, "s-b").unwrap();
    append_event(&b, &user_msg("İstanbul 的记录")).unwrap();
    append_event(&b, &assistant_msg("完全无关的中文内容")).unwrap();
    append_event(&b, &user_msg(&"标题超过四十个字符".repeat(6))).unwrap();
    write_title(&dir, "s-b", "旁车改的名").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));

    // s-d：标题截断路径专用——**首条**用户消息就超 40 字符，且不写旁车（旁车标题
    //      不截断，写了就盖掉截断结果）。文本刻意避开网格里的每个 query 词
    //      （rust/aa/中/的/İ/会议/istanbul），免得把它卷进命中面、让
    //      「命中会话 ≥2」这条对照的语义变浑。
    let d = session_file(&dir, "s-d").unwrap();
    append_event(&d, &user_msg(&"lorem ipsum dolor sit amet ".repeat(4))).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));

    // s-e：摘录省略号专用。生产侧 `if snippet.is_none()` 只留**首处**命中的摘录 ⇒
    //      "长前缀 + 命中词"必须落在同一行、且该行是本会话的首处命中。s-a 那条长前缀行
    //      不含 `rust`，而 s-a 的 rust 首处命中在前面那条 30 字符的短行上（前缀 11 <
    //      SNIPPET_CONTEXT_CHARS = 24 ⇒ 两端都不越界 ⇒ 没有省略号），所以光把长前缀行
    //      塞进 s-a 是走不到这一格的（首跑就红在这里，红因是夹具、不是生产码）。
    //      这里前缀 96 字符 > 24 ⇒ `from > 0` ⇒ 前省略号必然出现。
    let e = session_file(&dir, "s-e").unwrap();
    append_event(
        &e,
        &user_msg(&format!("{}rust 就在很后面", "lorem ipsum ".repeat(8))),
    )
    .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));

    // s-c：无用户消息 ⇒ 自动标题 None；命中只在 assistant 侧；含坏行与空行
    let c = session_file(&dir, "s-c").unwrap();
    append_event(&c, &assistant_msg("只有助手消息，里面提到 RUST 一次")).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&c)
        .unwrap()
        .write_all(b"{not-json\n\n")
        .unwrap();
    std::thread::sleep(std::time::Duration::from_millis(30));

    // 边界件：空文件 / 全坏行文件 / 非 jsonl 后缀 / 非法 id / undo 备份
    std::fs::write(session_file(&dir, "s-empty").unwrap(), "").unwrap();
    std::fs::write(session_file(&dir, "s-bad").unwrap(), "{oops\n").unwrap();
    std::fs::write(dir.join("note.txt"), "rust").unwrap();
    std::fs::write(dir.join("bad id.jsonl"), "rust").unwrap();
    std::fs::write(dir.join("s-a.jsonl.undo.bak"), "rust").unwrap();
    // 命中会话的旁车：标题覆盖 + 工作区透出
    write_title(&dir, "s-a", "甲会话").unwrap();
    write_workspace(&dir, "s-a", Some("F:/ws")).unwrap();

    // ① 列表：逐字段（含次序、事件数、标题截断、旁车覆盖、坏行计数）
    let got_list = list_sessions(&dir);
    assert_eq!(
        json(&got_list),
        json(&ref_list_sessions_old(&dir)),
        "list_sessions 与旧实现分岔"
    );
    // 目录不存在：两侧同为空表
    assert_eq!(
        json(&list_sessions(&dir.join("absent"))),
        json(&ref_list_sessions_old(&dir.join("absent")))
    );

    // ② 搜索：query × limit 网格（trim / 空查询 / 大小写 / 变长折叠 / 中文 / 早停）
    let grid: Vec<(&str, usize)> = vec![
        ("rust", 10),
        ("RUST", 10),
        ("  rust  ", 10),
        ("Rust", 1),
        ("aa", 10),
        ("会议", 10),
        ("istanbul", 10),
        ("İ", 10),
        ("的", 10),
        ("不存在的词", 10),
        ("", 10),
        ("   ", 5),
        ("rust", 1),
        ("rust", 0),
        ("中", 2),
    ];
    for (q, limit) in &grid {
        assert_eq!(
            json(&search_sessions(&dir, q, *limit)),
            json(&ref_search_sessions_old(&dir, q, *limit)),
            "search_sessions 分岔：query={q:?} limit={limit}"
        );
    }

    // 阳性对照：夹具真的把上面几档都走到了（否则"两边都空所以相等"是零证明力）
    assert!(got_list.len() >= 5, "夹具太少 = {}", got_list.len());
    assert!(
        got_list.iter().any(|m| m.title.is_none()),
        "没有'无用户消息 ⇒ 标题 None'的会话"
    );
    assert!(
        got_list
            .iter()
            .any(|m| m.title.as_deref() == Some("甲会话")),
        "旁车标题没覆盖上"
    );
    assert!(
        got_list.iter().any(|m| m
            .title
            .as_deref()
            .is_some_and(|t| t.chars().count() == SESSION_TITLE_CHARS)),
        "标题截断没走到"
    );
    let wide = search_sessions(&dir, "rust", 10);
    assert!(wide.len() >= 2, "命中会话不足两个 = {}", wide.len());
    assert!(
        wide.iter().any(|h| h.match_count > 1),
        "没有多命中的会话 ⇒ 非重叠计数没被测到"
    );
    assert!(
        wide.iter().any(|h| h.snippet.contains('…')),
        "没有带省略号的摘录 ⇒ 长前缀路径没被测到"
    );
    assert!(
        wide.iter().any(|h| h.title.as_deref() == Some("甲会话")),
        "命中结果里没带上旁车标题"
    );
    assert_eq!(
        search_sessions(&dir, "aa", 10)[0].match_count,
        1,
        "`aaa` 里找 `aa` 必须非重叠只算 1 次"
    );
    assert!(
        !search_sessions(&dir, "İ", 10).is_empty(),
        "变长小写折叠（İ → i + 组合点）必须仍能命中"
    );
    std::fs::remove_dir_all(&dir).ok();
}
