//! 相关性词表的**切分层**（队列 Q9 · ML 稿 §3.3）：从最近几条用户消息里切出
//! 候选词，交给宿主算 `relevance` 信号。
//!
//! 为什么单独一文件：它是一段与块组装无关的纯文本算术（ASCII 按空白标点切、
//! CJK 走 2 字滑窗），改它不该牵动监听器本体，读它也不该带上几百行注入逻辑。
//! 纪律与 `memory_search` 同源：**不建索引、无词典**——切分规则的全部知识就在这三函数里。

/// 相关性取数窗口 = 最近 N 条**用户**消息（稿 §3.3 的 N）。
const RELEVANCE_TURNS: usize = 3;
/// 词表上限：再多对 LIKE 计数没有区分度，只增加扫描成本。
const RELEVANCE_MAX_TERMS: usize = 8;
/// 短于此的词不进词表（`the` / `of` 一类 LIKE 命中面过宽 = 纯噪声）。
const RELEVANCE_MIN_LEN: usize = 3;

/// 从最近 [`RELEVANCE_TURNS`] 条用户消息取相关性关键词表（队列 Q9 · 稿 §3.3）。
///
/// **取数时刻 = 会话首查那一次**：本函数的调用点在会话预取缓存之内，一个会话只算
/// 一次 ⇒ 逐轮动态检索不改写 system 前缀，KV v2 的 M1 纪律不破（稿没写取数时刻，
/// 这是本批的第一条订正）。
///
/// 纪律与 `memory_search` 同源：**不建索引、无词典**。切分只有两档：① ASCII 侧按
/// 空白与常见标点切；② CJK 侧对连续汉字串取 **2 字滑窗**。②是对稿「不分词」那句的
/// **第二条订正**，也是让 L2 对中文真正有效的前提：本机用户主力语言是中文，而中文
/// 句子没有空格 ⇒ 纯空白切分会把整句当一个词，`LIKE '%整句%'` 恒不命中，relevance
/// 信号对中文永远为 0 —— 那等于交付一个永远不响的仪表。2 字滑窗仍是纯函数、零词典、
/// 零索引，与 `memory_search` 的 LIKE 同档。
pub(crate) fn relevance_terms(messages: &[session_store::Message]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut seen_users = 0usize;
    for m in messages.iter().rev() {
        if seen_users >= RELEVANCE_TURNS || out.len() >= RELEVANCE_MAX_TERMS {
            break;
        }
        let session_store::Message::User { content, .. } = m else {
            continue;
        };
        seen_users += 1;
        for term in split_terms(content) {
            if out.len() >= RELEVANCE_MAX_TERMS {
                break;
            }
            if !out.contains(&term) {
                out.push(term);
            }
        }
    }
    out
}

/// 单条消息 → 候选词表（ASCII 词 + CJK 2 字滑窗），按出现顺序、去重。
fn split_terms(content: &str) -> Vec<String> {
    let mut out = Vec::new();
    // 按"字母数字 / 汉字"分组：汉字连续段单独走 n-gram，其余走空白标点切
    for (is_cjk, seg) in group_runs(content) {
        if is_cjk {
            let chars: Vec<char> = seg.chars().collect();
            if chars.len() == 2 {
                // 两字词整词收下（滑窗对它退化成自己）
                out.push(seg.to_string());
            }
            for w in chars.windows(2) {
                out.push(w.iter().collect());
            }
        } else {
            for raw in seg.split(|c: char| {
                c.is_whitespace()
                    || matches!(
                        c,
                        ',' | '.'
                            | ';'
                            | ':'
                            | '!'
                            | '?'
                            | '('
                            | ')'
                            | '['
                            | ']'
                            | '{'
                            | '}'
                            | '/'
                            | '\\'
                    )
            }) {
                let t = raw.trim_matches(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-'));
                let n = t.chars().count();
                if n < RELEVANCE_MIN_LEN || t.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                out.push(t.to_lowercase());
            }
        }
    }
    out
}

/// 把字符串切成「连续汉字段 / 非汉字段」交替的片段（保持出现顺序）。
fn group_runs(content: &str) -> Vec<(bool, String)> {
    let mut out: Vec<(bool, String)> = Vec::new();
    for c in content.chars() {
        let cjk = ('\u{4e00}'..='\u{9fff}').contains(&c);
        match out.last_mut() {
            Some((flag, buf)) if *flag == cjk => buf.push(c),
            _ => out.push((cjk, c.to_string())),
        }
    }
    out
}
