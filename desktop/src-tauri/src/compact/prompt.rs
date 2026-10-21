//! 摘要提示词（上游**逐字**）与输出校验。
//!
//! 逐字来源 = 上游 `packages/core/src/session/compaction.ts:107-185`：
//! `SUMMARY_TEMPLATE` / `SUMMARY_RULES` / 共享五段 / 更新分支 / `NUDGE`。
//!
//! 提示词正文**住在资产文件**（`prompts/compact/*.md`，`include_str!` 编译期内嵌），
//! 与本仓既有惯例同形：改文案不动代码。
//!
//! 装配形态与上游**逐字相同**：共享五段是五个元素用 `\n\n` 连起来的，所以本仓把
//! 最后两段（"Do not continue the task or call tools." 与 "Return only the
//! structured summary…"）合成一个资产文件（内部同样是 `\n\n`）——拼出来的字节
//! 与上游完全一致，而文件数少两个。

/// 上游的 `SUMMARY_TEMPLATE`（资产文件；`\n` 结尾由 [`asset`] 去掉）。
fn template() -> &'static str {
    asset(include_str!("../prompts/compact/template.md"))
}
/// 上游的 `SUMMARY_RULES`。
fn rules() -> &'static str {
    asset(include_str!("../prompts/compact/rules.md"))
}
/// 共享五段的第一段（"Summarize only what…"）。
fn preamble() -> &'static str {
    asset(include_str!("../prompts/compact/preamble.md"))
}
/// 共享五段的最后两段（中间夹一个空行）。
fn epilogue() -> &'static str {
    asset(include_str!("../prompts/compact/epilogue.md"))
}
/// 首次压缩那一句（"You MUST summarize the conversation above…"）。
fn first() -> &'static str {
    asset(include_str!("../prompts/compact/first.md"))
}
/// 更新分支的第一句（"Update the existing checkpoint…"）。
fn update_head() -> &'static str {
    asset(include_str!("../prompts/compact/update-head.md"))
}
/// 更新分支的 legacy 变体（上游 `compaction.ts:169-173`）。
fn update_legacy() -> &'static str {
    asset(include_str!("../prompts/compact/update-legacy.md"))
}
/// 更新分支的后三段（"Newer history…" / "Incorporate…" / "Return only the updated…"）。
fn update_tail() -> &'static str {
    asset(include_str!("../prompts/compact/update-tail.md"))
}
/// 提醒那一句（上游 `compaction.ts:181-182` `NUDGE`）。
pub(crate) fn nudge() -> &'static str {
    asset(include_str!("../prompts/compact/nudge.md"))
}

/// 去掉资产文件末尾的换行。
///
/// 为什么需要：上游那几段常量**没有**末尾换行（它们是 TS 模板字符串），而资产
/// 文件被编辑器补一个行尾换行是很平常的事。不去掉的话拼出来的提示词与上游差
/// 一个 `\n`，"逐字节相等"这条判据就永远红。
fn asset(s: &str) -> &str {
    s.trim_end_matches('\n')
}

/// 共享五段（上游 `compaction.ts:149-155` 同款：`\n\n` 连接）。
fn shared() -> String {
    [preamble(), template(), rules(), epilogue()].join("\n\n")
}

/// 装配这一趟的摘要提示词。
///
/// `update` = 已有 checkpoint（上游 `buildPrompt(update)`）；`legacy` = 那个
/// checkpoint 是旧模板写的（上游只在旧摘要含 `## Additional Context` 时才置位
/// ——我们没有那个旧格式，所以生产路径恒为 `false`，但这一支仍按上游逐字实现，
/// 免得将来接上旧会话时才发现少了一段）。
pub(crate) fn build_prompt(update: bool, legacy: bool) -> String {
    let shared = shared();
    if !update {
        return [first(), shared.as_str()].join("\n\n");
    }
    let mut parts: Vec<&str> = vec![update_head()];
    if legacy {
        parts.push(update_legacy());
    }
    parts.push(update_tail());
    parts.push(shared.as_str());
    parts.join("\n\n")
}

/// 模板里的**标题行**（上游 `compaction.ts:287`：
/// `SUMMARY_TEMPLATE.split("\n").filter((line) => line.startsWith("##"))`）。
///
/// ⚠ 那个过滤**包含 `###` 开头的那三行**（`"### Completed".startsWith("##")` 为真）
/// ⇒ 一共 **10** 行，不是 7 行。少算三行 = 输出校验放宽 = 摘要缺了
/// 「已完成 / 进行中 / 受阻」也判通过。
pub(crate) fn headings() -> Vec<&'static str> {
    template().lines().filter(|l| l.starts_with("##")).collect()
}

/// 输出校验（上游 `compaction.ts:288` `filled`）：回复里**任一行 trim 后等于**
/// 模板里的某个标题行即通过（**OR 关系**：不要求全、不看内容）。
///
/// 为什么不要求十行齐：模型经常省掉不适用的一节（模板自己写着 "you may omit
/// sections that aren't applicable"），要求全齐 = 把合法的摘要判成不合格，
/// 而失败一次就多烧一条模型往返。
pub(crate) fn filled(text: &str) -> bool {
    let headings = headings();
    text.lines().any(|line| headings.contains(&line.trim()))
}
