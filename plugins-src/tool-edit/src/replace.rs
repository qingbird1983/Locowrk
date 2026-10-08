//! edit_replace：old_string → new_string。精确匹配优先（唯一 / replace_all）；
//! 精确零命中再进容错匹配——行尾空白、CRLF/LF、BOM 归一（模型打不出 \r、
//! 看不见尾随空格，这是「照抄也失败」的主流失败面）。缩进、大小写、编辑
//! 距离不做（误改风险）；唯一性是安全底线：不唯一不替换。
//!
//! 容错实现（docs/design/edit_replace.md §三·八）：把原文与 old_string 各自
//! 归一化成字节流（只删 ASCII 字节，多字节字符不动），在归一化流上做普通
//! 子串搜索，命中区间经 per-line kept 映射表映射回原缓冲字节区间后整体
//! 替换；new_string 的行尾随命中区域的行尾风格适配（CRLF 文件插入 LF 会
//! 造成混合行尾）。结果如实回报 `match_type`（exact/fuzzy），模糊命中是
//! 「你给的原文与实际有差异」的信号。

use std::path::Path;

use plugin_api::HostVtable;
use plugin_api::fsutil::{
    arg_expected_revision, backup_original, check_revision, content_revision, effective_roots,
    resolve_existing,
};
use plugin_api::serde_json::{Value, json};

pub(crate) fn edit_replace(args: &Value, host: *const HostVtable) -> Result<Value, String> {
    let path = args
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `path`".to_string())?;
    let old = args
        .get("old_string")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `old_string`".to_string())?;
    let new = args
        .get("new_string")
        .and_then(Value::as_str)
        .ok_or_else(|| "missing string field `new_string`".to_string())?;
    let replace_all = args
        .get("replace_all")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if old.is_empty() {
        return Err("edit_replace `old_string` must not be empty".into());
    }
    // ㉘·B 补口（2026-10-04 实机走查发现 v2.56 漏网）：编辑类也是路径类
    // 写面，围栏跟会话（vtable session_context 优先，env 回落）
    // SAFETY: host 是 init 时宿主交付的 'static vtable 指针（EditState 同契约）
    let roots = unsafe { effective_roots(host) }?;
    let resolved = resolve_existing(Path::new(path), &roots)?;
    let display = resolved.display().to_string();
    let content =
        std::fs::read_to_string(&resolved).map_err(|e| format!("edit_replace `{display}`: {e}"))?;

    // Revision Guard（可靠性稿 §3，G3）：编辑必须证明观察未过期——revision
    // 与当前内容（LF 归一哈希）不符即拒，报文指路重读；比对在任何匹配 /
    // 备份之前（fail fast，不动文件系统）。
    let expected = arg_expected_revision("edit_replace", args)?;
    check_revision(
        "edit_replace",
        &display,
        &expected,
        &content_revision(content.as_bytes()),
    )?;

    // 精确匹配优先：零命中才进容错匹配（精确多重命中 = 局面无歧义，报错
    // 列出行号让模型扩上下文，不浪费容错轮）
    let exact: Vec<usize> = content.match_indices(old).map(|(i, _)| i).collect();
    let (new_content, edits, match_type, first_span) = if !exact.is_empty() {
        if exact.len() > 1 && !replace_all {
            let lines: Vec<String> = exact
                .iter()
                .map(|&i| line_at(&content, i).to_string())
                .collect();
            return Err(format!(
                "edit_replace `old_string` matches {} times in `{display}` at lines {} — \
                 include more surrounding context to make it unique, or pass replace_all=true",
                exact.len(),
                lines.join(", ")
            ));
        }
        let take: Vec<usize> = if replace_all { exact } else { vec![exact[0]] };
        let spans: Vec<(usize, usize)> = take.iter().map(|&i| (i, i + old.len())).collect();
        (
            splice(&content, &spans, new),
            spans.len(),
            "exact",
            spans[0],
        )
    } else {
        match fuzzy_replace(&content, old, new, replace_all, &display)? {
            Some(r) => (r.new_content, r.edits, "fuzzy", r.first),
            None => {
                return Err(format!(
                    "edit_replace `old_string` not found in `{display}` \
                     (exact and whitespace/CRLF-insensitive search both failed) — \
                     fs_read the file and copy old_string character-for-character \
                     (indentation and line endings included); the file may also have \
                     changed since you last read it"
                ));
            }
        }
    };

    // 备份先行：失败则整体报错，目标不动
    let backup = backup_original(&resolved, "tool-edit")?;
    write_atomic(&resolved, &new_content)?;
    Ok(json!({
        "path": display,
        "bytes": new_content.len(),
        "edits": edits,
        "match_type": match_type,
        "byte_range": [first_span.0, first_span.1],
        "existed": true,
        "backup": backup.display().to_string(),
        "revision": content_revision(new_content.as_bytes()),
    }))
}

/// 字节偏移所在的 1-based 行号（报错定位用）。
fn line_at(content: &str, byte_idx: usize) -> usize {
    1 + content.as_bytes()[..byte_idx]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
}

/// 区间替换：按原缓冲坐标把 spans（必须升序不重叠）逐个换成 `new`。
fn splice(content: &str, spans: &[(usize, usize)], new: &str) -> String {
    let mut out = String::with_capacity(content.len() + new.len() * spans.len());
    let mut last = 0usize;
    for &(s, e) in spans {
        out.push_str(&content[last..s]);
        out.push_str(new);
        last = e;
    }
    out.push_str(&content[last..]);
    out
}

// ---- 容错匹配：归一化字节流 + 原文坐标映射 ----

/// 一行的原文位置 + 归一化形态。`norm` 只删字节不增不改序（剥 BOM/去 \r/
/// 去行尾空白），`kept` 给出 norm 每个字节在原缓冲中的偏移——命中区间由
/// 它映射回原文坐标整体替换，跳过的字节（尾随空格等）天然留在替换区间内。
struct LineMap {
    /// 行文本（不含行尾符）在原缓冲中的 [start, end)
    text: (usize, usize),
    /// 行尾符在原缓冲中的 [start, end)（无行尾符 = start == end）
    term: (usize, usize),
    /// 归一化行文本
    norm: Vec<u8>,
    /// norm 每字节的原缓冲偏移（与 norm 等长）
    kept: Vec<usize>,
}

/// 归一化文档：归一化字节流 + 每行在流中的起点（len = 行数 + 1，末元素 =
/// 流总长）+ 行映射表。
struct NormDoc {
    bytes: Vec<u8>,
    starts: Vec<usize>,
    lines: Vec<LineMap>,
}

fn build_norm_doc(buf: &str) -> NormDoc {
    let b = buf.as_bytes();
    let mut lines: Vec<LineMap> = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        let nl = b[i..]
            .iter()
            .position(|&c| c == b'\n')
            .map_or(b.len(), |o| i + o);
        let (t_end, term) = if nl < b.len() {
            if nl > i && b[nl - 1] == b'\r' {
                (nl - 1, (nl - 1, nl + 1))
            } else {
                (nl, (nl, nl + 1))
            }
        } else {
            (nl, (nl, nl))
        };
        let (norm, kept) = normalize_seg(&b[i..t_end], i, lines.is_empty());
        lines.push(LineMap {
            text: (i, t_end),
            term,
            norm,
            kept,
        });
        i = if nl < b.len() { nl + 1 } else { nl };
    }
    let mut bytes = Vec::with_capacity(buf.len());
    let mut starts = vec![0usize];
    for l in &lines {
        bytes.extend_from_slice(&l.norm);
        if l.term.0 < l.term.1 {
            bytes.push(b'\n');
        }
        starts.push(bytes.len());
    }
    NormDoc {
        bytes,
        starts,
        lines,
    }
}

/// 归一化一段行文本：剥 BOM（仅首行）、去全部 \r、去行尾空白。
fn normalize_seg(seg: &[u8], base: usize, first_line: bool) -> (Vec<u8>, Vec<usize>) {
    let mut start = 0usize;
    if first_line && seg.starts_with(b"\xEF\xBB\xBF") {
        start = 3;
    }
    let mut end = seg.len();
    while end > start && matches!(seg[end - 1], b' ' | b'\t' | b'\r') {
        end -= 1;
    }
    let (mut norm, mut kept) = (Vec::new(), Vec::new());
    for (k, &byte) in seg.iter().enumerate().take(end).skip(start) {
        if byte == b'\r' {
            continue;
        }
        norm.push(byte);
        kept.push(base + k);
    }
    (norm, kept)
}

/// 归一化坐标 → 原文坐标。终点落 `'\n'` 槽位（off == len）= 匹配停在行
/// 文本末、未吞行尾符 → 取行文本末（行尾空白随之进替换区间）；off ==
/// len + 1 只可能是吞了文档末行行尾（p == 总长）→ 取行尾符末。起点落
/// `'\n'` 槽位 = 匹配从行尾符开始吞 → 取行尾符首。
fn map_pos(doc: &NormDoc, p: usize, is_end: bool) -> usize {
    let last = doc.lines.len() - 1;
    let i = doc
        .starts
        .partition_point(|&s| s <= p)
        .saturating_sub(1)
        .min(last);
    let off = p - doc.starts[i];
    let l = &doc.lines[i];
    if off < l.norm.len() {
        l.kept[off]
    } else if is_end {
        if off == l.norm.len() + 1 && l.term.0 < l.term.1 {
            l.term.1
        } else {
            l.text.1
        }
    } else if l.term.0 < l.term.1 {
        l.term.0
    } else {
        l.text.1
    }
}

/// 归一化坐标所在的 1-based 行号（报错定位用）。
fn norm_line_at(doc: &NormDoc, p: usize) -> usize {
    let last = doc.lines.len() - 1;
    doc.starts
        .partition_point(|&s| s <= p)
        .saturating_sub(1)
        .min(last)
        + 1
}

/// 非重叠贪心子串搜索（字节级——归一化只动 ASCII，多字节字符不被拆）。
fn find_spans(hay: &[u8], needle: &[u8]) -> Vec<usize> {
    let (n, m) = (hay.len(), needle.len());
    let mut out = Vec::new();
    if m == 0 || n < m {
        return out;
    }
    let mut i = 0usize;
    while i + m <= n {
        if &hay[i..i + m] == needle {
            out.push(i);
            i += m;
        } else {
            i += 1;
        }
    }
    out
}

struct FuzzyHit {
    new_content: String,
    edits: usize,
    first: (usize, usize),
}

/// 容错匹配主入口。`Some` = 命中已拼好新内容；`None` = 容错也未命中。
fn fuzzy_replace(
    content: &str,
    old: &str,
    new: &str,
    replace_all: bool,
    display: &str,
) -> Result<Option<FuzzyHit>, String> {
    let doc = build_norm_doc(content);
    if doc.lines.is_empty() {
        return Ok(None); // 空文件无从命中
    }
    let old_doc = build_norm_doc(old);
    if old_doc.bytes.is_empty() {
        return Err(
            "edit_replace `old_string` contains only whitespace/line breaks — \
             nothing to match after whitespace-insensitive normalization"
                .into(),
        );
    }
    let hits = find_spans(&doc.bytes, &old_doc.bytes);
    if hits.is_empty() {
        return Ok(None);
    }
    if hits.len() > 1 && !replace_all {
        let lines: Vec<String> = hits
            .iter()
            .map(|&h| norm_line_at(&doc, h).to_string())
            .collect();
        return Err(format!(
            "edit_replace `old_string` matches {} times in `{display}` (fuzzy) at lines {} — \
             include more surrounding context to make it unique, or pass replace_all=true",
            hits.len(),
            lines.join(", ")
        ));
    }
    let spans: Vec<(usize, usize)> = hits
        .iter()
        .map(|&h| {
            (
                map_pos(&doc, h, false),
                map_pos(&doc, h + old_doc.bytes.len(), true),
            )
        })
        .collect();
    // new_string 行尾随命中区域适配：CRLF 区域插入 LF（或反之）都会造成
    // 混合行尾；命中区域行尾风格以 span 内/紧邻行尾符为准
    let adapted = if spans.iter().any(|&(s, e)| region_is_crlf(content, s, e)) {
        new.replace("\r\n", "\n").replace('\n', "\r\n")
    } else {
        new.replace("\r\n", "\n")
    };
    Ok(Some(FuzzyHit {
        new_content: splice(content, &spans, &adapted),
        edits: spans.len(),
        first: spans[0],
    }))
}

/// 命中区域的行尾风格判定：span 内含 \r\n，或 span 紧邻的前后行尾符是
/// \r\n（单行 span 自身不含行尾符时看前后）。
fn region_is_crlf(content: &str, s: usize, e: usize) -> bool {
    let b = content.as_bytes();
    b[s..e].windows(2).any(|w| w == b"\r\n")
        || b[e..].starts_with(b"\r\n")
        || (s >= 2 && &b[s - 2..s] == b"\r\n")
}

/// 原子写：临时文件（唯一名，同目录）+ rename，写一半崩溃不留损坏文件
/// （Windows rename 覆盖既有目标；错误文案 edit 语境，区别于 memlayout
/// 的 memory 语境——同一模式暂未共享，第二个消费方出现时上收 fsutil）。
fn write_atomic(target: &Path, data: &str) -> Result<(), String> {
    use std::sync::atomic::{AtomicUsize, Ordering};
    static SEQ: AtomicUsize = AtomicUsize::new(0);
    let n = SEQ.fetch_add(1, Ordering::Relaxed);
    let tmp = target.with_extension(format!("tmp{}", n));
    std::fs::write(&tmp, data)
        .map_err(|e| format!("edit_replace temp write `{}`: {e}", tmp.display()))?;
    match std::fs::rename(&tmp, target) {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = std::fs::remove_file(&tmp);
            Err(format!(
                "edit_replace atomic write `{}`: {e}",
                target.display()
            ))
        }
    }
}
