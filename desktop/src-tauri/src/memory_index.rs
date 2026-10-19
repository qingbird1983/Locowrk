//! 记忆**索引**注入（MR 稿 §4.1 队列 Q2 起，S3 刀① 起管两层）。
//!
//! **这块治什么**：旧形状注入的是 `<persistent_memory>`——全局层事实**正文** top-K，
//! 而全局层实测常年为空（`memories` 表 0 行）；项目层明明有货，模型也**看不见**，它得先
//! 想到要调 `memory_list` 才知道本项目记了什么。MR 稿 §0 那条真实失败的第三条根因就是
//! 这个：「注入面三重盲」。本件把**指针级索引**放进 system，让新会话第一句话之前就知道
//! "记了哪些条、各在哪一篇"。刀① 之后全局层也走这里（旧正文块退役），所以本件同时给
//! [`LEAD_IN_PROJECT`] 与 [`LEAD_IN_GLOBAL`] 两条导语，由 `memory/source.rs` 按层挑。
//!
//! **注入的是指针不是正文**（与 v2.136「项目层事实不自动注入」裁定不冲突，
//! 那条禁的是把事实全文灌进 system）：一行一条 `- <key> — <hook>`，正文仍按需
//! `memory_read`。这与 ZCode 的 `MEMORY.md` 常驻索引同构（MR 稿 §1.2）。
//!
//! 数据源**只有一个**（S1 刀②c-1 起）：`<数据根>/memory/{global|projects/<项目目录>}/MEMORY.md`
//! 里的指针行。文件缺失或只有 `# 记忆索引` 空壳标题（2026-10-12 本机实测就是这个形态：
//! `TITY` 目录里根本没有 `MEMORY.md`，另外三个项目都只有空壳）⇒ **整节如实缺席**。
//! 真源就是文件，"文件里没有指针行"等于"这一层没记过"——先前还有一档"人类层没接线
//! 才由 sqlite 该键位的行现合成"，刀②c-1 连同 `MemoryStore::index_rows` 一起删了：
//! 库里那份副本不再是同一件事的答案，而"没接线"这个状态本身也没了。
//!
//! 预算（稿 §6）：200 行 / 25,000 字符，与写侧**同一份常量**（`memlayout`，SS10 三档盯着），
//! 超限附的那句 WARNING 也与写侧**同一串**；文件里本来就带着这句时（写侧钳出来的）
//! 这里要**认出来并透传**——`pointer_lines_from_md` 只收指针行，先前会把这句剥掉，
//! 于是"人打开文件夹看到还有更多、模型拿到的却看起来完整"。陈旧标注 `（N 天前）`
//! 由注入侧按 `updated_at` 现算，>48 h 才标，**不写进文件**（新鲜度只有 mtime/updated_at
//! 一个信号，稿 §2 纪律 5）。
//!
//! 字节钉死不在本件里做：整块随 [`crate::memory`] 的 `MemorySource` 交付，
//! 吃 `MemoryInject` 既有的会话预取缓存（键 = `(session_id, scope_id)`）。

use std::collections::HashMap;
use std::path::Path;

/// 索引预算的**唯一出处在 `plugin_api::memlayout`**（写侧落盘的 `MEMORY.md` 与
/// 注入侧共用同一份数字——两头各写一份迟早会漂成"文件里 210 行、注入只给看 200 行"）。
/// 这里只做再导出，让本模块与它的测试继续按裸名取用。
pub(crate) use plugin_api::memlayout::{MAX_INDEX_CHARS, MAX_INDEX_LINES};
/// 陈旧标注门槛：48 h（稿 §6「>48 h 才标」）。
pub(crate) const STALE_AFTER_SECS: i64 = 48 * 3600;

/// 项目段索引导语（给模型的说明）。S3 刀① 起导语**按层给**——同一段话写在
/// "本项目"上对全局层是假话（全局层跨项目成立），所以两档各写一条，由调用方
/// （[`crate::memory::source`]）挑，`from_md` 自己不猜。
pub(crate) const LEAD_IN_PROJECT: &str =
    "Project memory index — pointers only, never the bodies (one line \
     per saved fact of THIS project). Read a value with memory_read(key=…), search with \
     memory_search. Empty results here mean this project has no saved facts, not that it \
     has no history — for past work use memory_project_history.";

/// 全局段索引导语（S3 刀① 新增，方案稿 §A④ 里"两个索引"的那一个）。
pub(crate) const LEAD_IN_GLOBAL: &str =
    "Global memory index — pointers only, never the bodies (one line \
     per saved fact that holds across ALL projects). Read a value with memory_read(key=…), \
     search with memory_search. An empty section here means nothing has been saved as a \
     standing preference, not that this user has no history.";

/// 超限时的指路行（截断必须自描述，否则模型以为"就这些"）。
///
/// 刀②b-3 起它的**值 = 写侧那一句**（`memlayout::INDEX_TRUNCATED_NOTE`）：原先
/// "文件里写一句给人看、注入里另写一句给模型看"= 同一件事两个出处，正是这一刀要
/// 消掉的暂时状态。裸名 `TRUNCATED_NOTE` 保留，本模块与它的测试继续按旧名取用。
const TRUNCATED_NOTE: &str = plugin_api::memlayout::INDEX_TRUNCATED_NOTE;

/// 由 `MEMORY.md` 投影合成索引内文（**唯一一档**）。
///
/// 返回 `None` = 这一节缺席（读不到 / 只有标题 / 一行指针都没有）。"文件里没指针行"
/// 就是**这个项目确实没记过**，如实缺席，不去库里找一份已不再是真源的副本。
///
/// 两件必须在这里做的事：
/// ① **认出文件里那句截断说明并透传**：`pointer_lines_from_md` 只收 `- ` 开头的指针行，
///    那句 WARNING 会被剥掉 ⇒ 若不当标记处理，索引被截断这件事到模型那一侧就消失了
///    （正是"人打开文件夹看到截断、模型看不到"的那种分叉）。
/// ② **按 `ages` 现算陈旧标注**（`- [stem](stem.md) — hook` 里那个 stem 对应的事实
///    最近改动时间）。这一维必须站在注入侧现算，不能由写侧落进文件：落盘时写死的
///    "3 天前"在没人再写它的时候**永远不会自己变陈旧**。刀②b-3 把读源扳到 md 时
///    把这条静默丢了（当时只有已退役的 `from_rows` 那档在算），刀②c-1 补回并留测试。
pub(crate) fn from_md(
    md_body: Option<&str>,
    ages: &HashMap<String, i64>,
    now: i64,
    lead_in: &str,
) -> Option<String> {
    let body = md_body?;
    let truncated_by_file = body.lines().any(|l| l.trim_end() == TRUNCATED_NOTE);
    let mut lines = pointer_lines_from_md(body)?;
    for line in &mut lines {
        // `unwrap_or_default` = 对不上号就不标（宁缺毋假）：人可以在索引里写自由的一行，
        // 而"这行没有新鲜度"与"这行是新的"是两个不同的话。
        let marker = stem_of(line)
            .and_then(|stem| ages.get(stem))
            .map(|at| stale_marker(*at, now))
            .unwrap_or_default();
        line.push_str(&marker);
    }
    finish(lines, truncated_by_file, lead_in)
}

/// 从一行指针文本里取出文件名主干（`- [x](x.md) — hook` → `x`）。
///
/// 取的是**链接目标**而不是方括号里的显示名：`refresh_index` 按磁盘文件名写这一行，
/// 而 key 与文件名主干可以不同（key 里的非法字符被安全化、Windows 保留名前缀 `_`），
/// 所以能与事实文件对上的只有那个 `(...md)`。不是这个形状的行 = `None`。
fn stem_of(line: &str) -> Option<&str> {
    let tail = line.split("](").nth(1)?;
    let target = tail.split(')').next()?;
    target.strip_suffix(".md")
}

/// 某一层「文件名主干 → 最近改动时间」（陈旧标注的输入）。
///
/// 按 **stem** 建表而不是按 key：索引行里写的是文件名，而 key 与文件名主干可以不同
/// （安全化 / Windows 保留名前缀 `_`）。取这一层走 `memlayout::facts_of`（按键位算目录）
/// 而不是 `iter_facts` 之后按 `workspace` 字段过滤——那个字段在人新建的无 frontmatter
/// 文件上只是目录名兜底，按它过滤会把这种文件漏掉。
pub(crate) fn ages_of_layer(root: &Path, workspace: &str) -> Result<HashMap<String, i64>, String> {
    let mut map = HashMap::new();
    for fact in plugin_api::memlayout::facts_of(root, workspace)? {
        if let Some(stem) = fact.path.file_stem().and_then(|s| s.to_str()) {
            map.insert(stem.to_string(), fact.updated_at);
        }
    }
    Ok(map)
}

/// 共用的收尾：导语 + 钳制 + 截断指路。
///
/// `lead_in` = 这一层的导语（[`LEAD_IN_PROJECT`] / [`LEAD_IN_GLOBAL`]）。它**由调用方
/// 传进来而不是在这里按参数猜**：两层的指针行形状完全相同，能区分它们的只有这句话，
/// 让 `finish` 自己判"是哪一层"就要多引一个层身份参数——那才是把一件事拆成两处问。
///
/// `truncated_by_file` = 文件里本来就带着那句截断说明（写侧钳出来的）。它和
/// "注入侧这一轮又钳了一次"**取并集**：两条都必须写那句话，否则——只认注入侧那一半
/// 的话，文件被写侧截断而注入侧没超限时，模型看到的是一份**看起来完整**的索引。
/// 钳制本身留着不是冗余：人可以把 `MEMORY.md` 手工编辑成超出预算的形状，
/// 预算保护必须站在注入通路上（数字与文案都只有一个出处 = `memlayout`）。
fn finish(lines: Vec<String>, truncated_by_file: bool, lead_in: &str) -> Option<String> {
    if lines.is_empty() {
        return None;
    }
    let (kept, clamped) = clamp(lines);
    let truncated = clamped || truncated_by_file;
    let mut out = String::from(lead_in);
    for l in kept {
        out.push('\n');
        out.push_str(&l);
    }
    if truncated {
        out.push('\n');
        out.push_str(TRUNCATED_NOTE);
    }
    Some(out)
}

/// 从 `MEMORY.md` 里取指针行（`- [Title](file.md) — hook` 形态，与
/// `plugin_api::memlayout::refresh_index` 的写出格式同源——**照生产构造代码抄**）。
///
/// `None` = 一个指针行都没有（只有标题 / 全是注释 / 空文件）。参数是 `&str` 而不是
/// `Option<&str>`：文件读不到那一档由调用方（`from_md` 的 `md_body?`）处理，
/// 在这里再套一层 Option 只会让"没有文件"与"文件里没指针行"两个意思混成一个。
fn pointer_lines_from_md(body: &str) -> Option<Vec<String>> {
    let mut out = Vec::new();
    let mut in_html_comment = false;
    for raw in body.lines() {
        let line = raw.trim_end();
        // 剥 HTML 注释（人写的批注不许进上下文，稿 §1.1 同款清洗）
        if in_html_comment {
            if line.contains("-->") {
                in_html_comment = false;
            }
            continue;
        }
        if line.starts_with("<!--") {
            if !line.contains("-->") {
                in_html_comment = true;
            }
            continue;
        }
        if line.starts_with("# ") || line.trim().is_empty() {
            continue;
        }
        if line.starts_with("- ") {
            out.push(line.to_string());
        }
    }
    (!out.is_empty()).then_some(out)
}

/// 行数与字符双钳：任一超限即截断并附 WARNING。
fn clamp(mut lines: Vec<String>) -> (Vec<String>, bool) {
    let mut truncated = false;
    if lines.len() > MAX_INDEX_LINES {
        lines.truncate(MAX_INDEX_LINES);
        truncated = true;
    }
    let mut budget = MAX_INDEX_CHARS;
    let mut kept: Vec<String> = Vec::with_capacity(lines.len());
    for l in lines {
        let cost = l.chars().count() + 1; // +1 = 换行
        if cost > budget {
            truncated = true;
            break;
        }
        budget -= cost;
        kept.push(l);
    }
    (kept, truncated)
}

/// 陈旧标注（>48 h 才标；未来时间戳 = 不标，不发明负数天数）。
fn stale_marker(updated_at: i64, now: i64) -> String {
    let age = now.saturating_sub(updated_at);
    if age <= STALE_AFTER_SECS {
        return String::new();
    }
    let days = age / 86_400;
    format!("（{days} days old）")
}

#[cfg(test)]
mod tests;
