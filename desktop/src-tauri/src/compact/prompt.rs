//! 主层压缩的提示词装配 + 摘要验收（S2 刀④b）。
//!
//! 四份稿子都是**资产文件**（`prompts/compact/*.md`，`include_str!` 编译期内嵌），
//! 与本仓既有惯例同形（内核六节静态段、记忆树 `prompts/{manager,summarizer}/`）：
//! 提示词正文不写在 Rust 字符串里，改文案不动代码。
//!
//! 三件结构上的讲究：
//!
//! 1. **两套逐字条款各住一份文件**（AC-S2-7 要的"不许合成一句话"的结构化写法）。
//!    `keep-security.md` 管"话说得多重"（安全约束 / 用户原话 / 下一步照抄，
//!    逐字对齐 ZCode `compact/prompt.ts` 的三处条款），`keep-verbatim.md` 管
//!    "指得准"（路径 / 符号 / 命令 / 错误串 / URL / 标识符，对齐 OpenCode
//!    `SUMMARY_RULES`）。两件事的对象不同，合成一段就会有一件被另一件挤掉——
//!    表现是摘要里路径都 paraphrase 成中文描述、或者安全禁令被写成"用户提过要求"。
//!    判据 = 用例 `the_two_preservation_clauses_stay_two_files`（把任一套并成一句时红）。
//! 2. **不许调工具这条说两遍**（开头 + 结尾），照 ZCode `prompt.ts` 的
//!    `NO_TOOLS_PREAMBLE` / `NO_TOOLS_TRAILER` 形状——同一份稿子复用两次，
//!    不复制第二份文本（SS20b 盯住"四份稿子各只被 include 一次"）。
//! 3. **七节节名只有一份**：模板与验收都读 [`super::SECTIONS`]，两边各写一遍
//!    节名的话，改一处 = 摘要永远"缺节"而没人知道为什么。

use session_store::Message;

use super::{SECTIONS, SUBSECTIONS, TOOL_OUTPUT_MAX_CHARS};

// 四份稿子按 `pub(crate)` 露出**名字**（不是再抄一遍文本）：用例要逐字节断言
// "骨架没被自定义指令改动"，只认这几份资产本身，不在测试里硬编码任何提示词字。
pub(crate) const NO_TOOLS: &str = include_str!("../prompts/compact/no-tools.md");
pub(crate) const TEMPLATE: &str = include_str!("../prompts/compact/template.md");
pub(crate) const KEEP_SECURITY: &str = include_str!("../prompts/compact/keep-security.md");
pub(crate) const KEEP_VERBATIM: &str = include_str!("../prompts/compact/keep-verbatim.md");

/// 装配本次摘要调用的 system 正文。`custom` = 设置里的自定义压缩指令
/// （空串 / 全空白 = 不加，与"没设这枚"同形）。
pub(crate) fn build_prompt(custom: Option<&str>) -> String {
    let mut out = String::new();
    out.push_str(NO_TOOLS);
    out.push_str("\n\n");
    out.push_str(TEMPLATE);
    out.push_str("\n\n");
    out.push_str(KEEP_SECURITY);
    out.push('\n');
    out.push_str(KEEP_VERBATIM);
    if let Some(extra) = custom.map(str::trim).filter(|s| !s.is_empty()) {
        out.push_str("\n\n# 附加指令（用户自己写的，优先级高于上面的写法偏好）\n");
        out.push_str(extra);
    }
    // 尾部再钉一次：摘要请求的正文是一大段历史，"只出文本"离得越近越管用
    out.push_str("\n\n");
    out.push_str(NO_TOOLS);
    out
}

/// 把要摘要的那段消息列转成喂模型的纯文本。
///
/// 三件必须做对的地方（都是"漏了就丢信息"的形状）：附件留名字与 mime（图我看不进去，
/// 但下一个自己要知道当时贴了什么）；assistant 的**调用意图**要写出来（工具名 + 参数，
/// 只写结果就等于把"为什么跑这条命令"丢了）；单条工具输出封顶
/// [`TOOL_OUTPUT_MAX_CHARS`]——摘要请求本身也占窗口，不封顶就是"为了压缩先撑爆"。
pub(crate) fn render_history(messages: &[Message]) -> String {
    let mut out = String::new();
    for m in messages {
        match m {
            Message::User { content, parts } => {
                out.push_str("[用户] ");
                out.push_str(content.trim());
                for p in parts {
                    match p {
                        session_store::ContentPart::Text { text } => {
                            if !text.trim().is_empty() {
                                out.push_str(" | ");
                                out.push_str(text.trim());
                            }
                        }
                        other => {
                            if let Some(a) = other.asset() {
                                out.push_str(&format!(" | 附件：{}（{}）", a.name, a.mime));
                            }
                        }
                    }
                }
                out.push('\n');
            }
            Message::Assistant {
                content,
                tool_calls,
            } => {
                out.push_str("[助手] ");
                out.push_str(content.trim());
                for c in tool_calls {
                    out.push_str(&format!(
                        " | 调用 {}({})",
                        c.tool,
                        clip(&c.args.to_string())
                    ));
                }
                out.push('\n');
            }
            Message::ToolResult {
                tool,
                output,
                error,
                ..
            } => match error {
                Some(e) => out.push_str(&format!("[工具 {tool} 失败] {}\n", clip(e))),
                None => out.push_str(&format!(
                    "[工具 {tool} 结果] {}\n",
                    clip(&output.to_string())
                )),
            },
        }
    }
    out
}

/// overflow 递减：把已经渲染好的那段历史**按比例缩到较新的那一截**（刀④d）。
///
/// 方向是"丢最旧"而不是"截尾巴"：摘要最要保住的是"最近在做什么、下一步是什么"，
/// 最旧的一头先出列（与 OpenCode `compaction.ts:95` 的 `SHRINK_STEPS` 同一方向）。
/// 切点只落在**整行**边界上——[`render_history`] 一行就是一条消息/一行结果，
/// 劈开一行会给模型半句不存在的话。缩掉的段就不再被摘要，这是应急档的代价，
/// 所以调用方要如实记下试到第几档（`Outcome::Failed::attempts`）。
pub(crate) fn shrink_history(text: &str, ratio: f64) -> String {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    let keep = ((lines.len() as f64) * ratio.clamp(0.0, 1.0)).ceil() as usize;
    let keep = keep.clamp(1, lines.len());
    lines[lines.len() - keep..].join("\n")
}

/// 封顶一行（按**字符**，不是字节——这条上限的参照 OpenCode 也是字符数）。
fn clip(s: &str) -> String {
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i >= TOOL_OUTPUT_MAX_CHARS {
            out.push_str("…（已截断，原文在会话日志里）");
            break;
        }
        out.push(ch);
    }
    out
}

/// 摘要正文里**缺哪些节**（验收 = [`super::MAX_COMPACT_FAILURES`] 那一档的输入）。
/// 判定口径：`#` 打头、剥掉 # 与空白后以节名开头即算这一节在场；节里除空白外
/// 得有内容——写「（无）」算有内容（那是如实回答，不是漏写）。父节的"有内容"
/// 包含它的子节（「工作状态」下面只有三个子节标题 = 齐）。
pub(crate) fn missing_sections(text: &str) -> Vec<&'static str> {
    let lines: Vec<&str> = text.lines().collect();
    let mut missing: Vec<&'static str> = Vec::new();
    for name in SECTIONS {
        let Some(at) = lines.iter().position(|l| is_heading(l, name)) else {
            missing.push(name);
            continue;
        };
        // body 到**同级或更高级**的标题行为止（父节的子节标题算父节的内容，不算结束）：
        // 把任意 `#` 都当结束，会让「## 工作状态」紧接「### 已完成」时被误判成空节——
        // 表现是"摘要明明齐了，却永远点名工作状态，压完还叠一次失败计数"。
        let level = heading_level(lines[at]);
        let end = (at + 1..lines.len())
            .find(|&i| {
                let l = heading_level(lines[i]);
                l != 0 && l <= level
            })
            .unwrap_or(lines.len());
        if !lines[at + 1..end].iter().any(|l| !l.trim().is_empty()) {
            missing.push(name);
        }
    }
    // 「工作状态」的三个子节各算一次：只写父节标题、里面空着 = 状态没交代
    for name in SUBSECTIONS {
        if !lines.iter().any(|l| is_heading(l, name)) {
            missing.push(name);
        }
    }
    missing
}

fn is_heading(line: &str, name: &str) -> bool {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix('#') else {
        return false;
    };
    rest.trim_start_matches('#').trim_start().starts_with(name)
}

/// 标题级别 = 行首 `#` 的个数；不是标题行返回 0（0 因此兼作"这一行不是标题"）。
fn heading_level(line: &str) -> usize {
    let trimmed = line.trim_start();
    if !trimmed.starts_with('#') {
        return 0;
    }
    trimmed.chars().take_while(|c| *c == '#').count()
}

/// 归一化模型给的摘要正文：剥最外层代码围栏、收敛连续空行、去首尾空白。
/// 只做**字节层面**的收拾，不改一个字的内容（改内容 = 拿摘要再去压缩摘要）。
pub(crate) fn format_summary(raw: &str) -> String {
    let mut text = raw.trim();
    if let Some(stripped) = text.strip_prefix("```") {
        // 只有整段被围栏包住时才剥（模型偶尔把 markdown 包进 code block）
        if let Some(end) = stripped.rfind("```") {
            let mut body = &stripped[..end];
            // ``` 之后同一行的语言标记（```markdown → "markdown"）随围栏一起走：
            // 它是围栏的一部分，留在正文第一行会被下游当成摘要内容。
            // 判据保守——那一行只剩一串非空白字符才算标记；含空格的行一律当正文留着。
            if let Some(nl) = body.find('\n') {
                let first = &body[..nl];
                if !first.is_empty() && !first.chars().any(char::is_whitespace) {
                    body = &body[nl + 1..];
                }
            }
            text = body.trim();
        }
    }
    let mut out = String::new();
    let mut blank_run = 0usize;
    for line in text.lines() {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run >= 2 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out.trim().to_string()
}
