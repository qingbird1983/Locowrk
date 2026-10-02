/**
 * unified diff 解析（UI shell 文档 §3.28，v2.53）：`edit_diff` 的
 * args.patch 文本 → 行级结构（审阅页签渲染用）。容错优先：解析不出的行
 * 按 `ctx` 归档，不抛错——审阅面是只读的，宁可多显示一行也不炸页签。
 */

export interface DiffLine {
  type: "add" | "del" | "ctx";
  oldNo?: number;
  newNo?: number;
  text: string;
}

export interface DiffHunk {
  oldStart: number;
  oldCount: number;
  newStart: number;
  newCount: number;
  lines: DiffLine[];
}

export interface ParsedDiff {
  /** `+++` 侧路径（b/ 前缀剥除）；解析不出 = null。 */
  path: string | null;
  hunks: DiffHunk[];
  added: number;
  removed: number;
}

/** 空文件侧的哨兵路径（/dev/null）。 */
const DEV_NULL = "/dev/null";

export function parseUnifiedDiff(text: string): ParsedDiff {
  const out: ParsedDiff = {
    path: null,
    hunks: [],
    added: 0,
    removed: 0,
  };
  const lines = text.split("\n");
  let i = 0;
  let cur: DiffHunk | null = null;
  let oldNo = 0;
  let newNo = 0;

  while (i < lines.length) {
    const line = lines[i];
    if (line.startsWith("+++ ")) {
      const p = line.slice(4).trim();
      out.path = p === DEV_NULL ? null : stripPrefix(p);
    } else if (line.startsWith("@@")) {
      // @@ -l,c +l,c @@ ...（尾部可有上下文标题）
      const m = /^@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@/.exec(line);
      if (m) {
        oldNo = Number(m[1]);
        newNo = Number(m[3]);
        cur = {
          oldStart: oldNo,
          oldCount: m[2] !== undefined ? Number(m[2]) : 1,
          newStart: newNo,
          newCount: m[4] !== undefined ? Number(m[4]) : 1,
          lines: [],
        };
        out.hunks.push(cur);
      }
    } else if (cur !== null) {
      if (line.startsWith("+")) {
        cur.lines.push({ type: "add", newNo, text: line.slice(1) });
        out.added += 1;
        newNo += 1;
      } else if (line.startsWith("-")) {
        cur.lines.push({ type: "del", oldNo, text: line.slice(1) });
        out.removed += 1;
        oldNo += 1;
      } else if (line.startsWith(" ") || line === "") {
        cur.lines.push({ type: "ctx", oldNo, newNo, text: line.slice(1) });
        oldNo += 1;
        newNo += 1;
      } else if (line.startsWith("\\")) {
        // "\ No newline at end of file"：无行号语义，跳过
      } else {
        // 容错：hunk 外/未知标记按上下文归档
        cur.lines.push({ type: "ctx", oldNo, newNo, text: line });
        oldNo += 1;
        newNo += 1;
      }
    }
    i += 1;
  }
  return out;
}

/** 剥 a/ 与 b/ 前缀（git 风格）；其余原样。 */
function stripPrefix(p: string): string {
  return p.startsWith("b/") ? p.slice(2) : p;
}

/** 折叠上下文（§3.28）：连续 ctx 段 >2×COLLAPSE+1 时中间折叠。输出 =
 * 可直接渲染的段序列（show 组 / collapsed 组）。 */
export const COLLAPSE = 3;

export type RenderBlock<D> =
  | { kind: "lines"; lines: D[] }
  | { kind: "collapsed"; lines: D[]; count: number };

export function collapseContext<D extends { type: DiffLine["type"] }>(
  lines: D[],
): RenderBlock<D>[] {
  const blocks: RenderBlock<D>[] = [];
  let ctxRun: D[] = [];
  const flush = () => {
    if (ctxRun.length === 0) return;
    if (ctxRun.length > COLLAPSE * 2) {
      blocks.push({ kind: "lines", lines: ctxRun.slice(0, COLLAPSE) });
      const mid = ctxRun.length - COLLAPSE * 2;
      blocks.push({ kind: "collapsed", lines: ctxRun.slice(COLLAPSE, ctxRun.length - COLLAPSE), count: mid });
      blocks.push({ kind: "lines", lines: ctxRun.slice(ctxRun.length - COLLAPSE) });
    } else {
      blocks.push({ kind: "lines", lines: ctxRun });
    }
    ctxRun = [];
  };
  for (const l of lines) {
    if (l.type === "ctx") ctxRun.push(l);
    else {
      flush();
      blocks.push({ kind: "lines", lines: [l] });
    }
  }
  flush();
  return blocks;
}
