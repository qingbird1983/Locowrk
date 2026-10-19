/** unified diff 解析（设计稿 §7.1「差异表」）：把写操作「改了什么」摊成人话，
 * 取代旧版「写入类展开只给属性行，看不到内容」。
 *
 * 只认四种行：hunk 头 `@@ … @@`、加行 `+`、删行 `-`、上下文行（空格开头）。
 * `diff --git` / `---` / `+++` / `\ No newline at end of file` 一律跳过——
 * 它们是文件头与元信息，不是内容变更。
 *
 * 解析失败（没有 hunk 头，或一行可认的都没有）返回 null，调用方整体降级成
 * `text` 块——**不把没解析成功的原始 diff 当代码展示**。 */

export interface DiffRow {
  kind: "hunk" | "add" | "del" | "ctx";
  /** 旧文件行号；加行与 hunk 头为 null。 */
  oldNo: number | null;
  /** 新文件行号；删行与 hunk 头为 null。 */
  newNo: number | null;
  text: string;
}

const HUNK = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/;

export function parseUnifiedDiff(raw: string): DiffRow[] | null {
  if (typeof raw !== "string" || !raw.trim()) return null;
  const rows: DiffRow[] = [];
  let oldNo = 0;
  let newNo = 0;
  let seenHunk = false;

  for (const line of raw.split(/\r?\n/)) {
    if (
      line.startsWith("diff --git ") ||
      line.startsWith("--- ") ||
      line.startsWith("+++ ") ||
      line.startsWith("\\ No newline")
    ) {
      continue;
    }
    const m = HUNK.exec(line);
    if (m) {
      oldNo = Number(m[1]);
      newNo = Number(m[2]);
      seenHunk = true;
      rows.push({ kind: "hunk", oldNo: null, newNo: null, text: line });
      continue;
    }
    if (line === "") continue;
    const body = line.slice(1);
    if (line[0] === "+") rows.push({ kind: "add", oldNo: null, newNo: newNo++, text: body });
    else if (line[0] === "-") rows.push({ kind: "del", oldNo: oldNo++, newNo: null, text: body });
    else if (line[0] === " ")
      rows.push({ kind: "ctx", oldNo: oldNo++, newNo: newNo++, text: body });
  }
  return seenHunk && rows.length > 0 ? rows : null;
}

/** 拿不到 patch 时的退化构造（`edit_replace` 只有 old/new 两段文本、
 * `edit_lines` 只知道被替换的行区间）：只摊「删了这些 / 加了这些」。
 * 行号区间已知的照实标，**不编造上下文行**。 */
export function diffFromReplacement(
  oldText: string,
  newText: string,
  firstOldLine: number | null,
): DiffRow[] {
  const rows: DiffRow[] = [];
  for (const [i, text] of (oldText ? oldText.split(/\r?\n/) : []).entries()) {
    rows.push({
      kind: "del",
      oldNo: firstOldLine === null ? null : firstOldLine + i,
      newNo: null,
      text,
    });
  }
  for (const text of newText ? newText.split(/\r?\n/) : []) {
    rows.push({ kind: "add", oldNo: null, newNo: null, text });
  }
  return rows;
}
