// Pure row model for the diff page: file/hunk headers, unified lines and
// side-by-side pairs. The page owns no business state beyond folds and mode.
import type { DiffDoc, DiffFile, DiffHunk, DiffLine } from "../mygo";

export type Mode = "side" | "unified";

export interface Row {
  kind: "file" | "hunk" | "line" | "meta";
  key: string;
  file?: DiffFile;
  hunk?: DiffHunk;
  hunkIndex?: number;
  line?: DiffLine;
  left?: DiffLine;
  right?: DiffLine;
  text?: string;
}

export function fileKey(file: DiffFile): string {
  return "f:" + file.path;
}

export function hunkKey(file: DiffFile, index: number): string {
  return "h:" + file.path + ":" + index;
}

// buildRows flattens a document for one view mode. Folds hide everything
// below the header they belong to.
export function buildRows(doc: DiffDoc, mode: Mode, folded: ReadonlySet<string>): Row[] {
  const rows: Row[] = [];
  for (const file of doc.files) {
    rows.push({ kind: "file", key: fileKey(file), file });
    if (folded.has(fileKey(file))) continue;
    file.hunks.forEach((hunk, index) => {
      rows.push({ kind: "hunk", key: hunkKey(file, index), file, hunk, hunkIndex: index });
      if (folded.has(hunkKey(file, index))) return;
      if (mode === "unified") {
        pushUnified(rows, file, hunk);
      } else {
        pushSide(rows, file, hunk);
      }
    });
  }
  return rows;
}

function pushUnified(rows: Row[], file: DiffFile, hunk: DiffHunk): void {
  hunk.lines.forEach((line, index) => {
    rows.push({ kind: "line", key: `${file.path}:${hunk.oldStart}:${index}`, file, hunk, line });
  });
}

function pushSide(rows: Row[], file: DiffFile, hunk: DiffHunk): void {
  const lines = hunk.lines;
  let i = 0;
  let seq = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (!line) break;
    if (line.kind === "del") {
      const dels: DiffLine[] = [];
      while (i < lines.length && lines[i]?.kind === "del") dels.push(lines[i++] as DiffLine);
      const adds: DiffLine[] = [];
      while (i < lines.length && lines[i]?.kind === "add") adds.push(lines[i++] as DiffLine);
      const n = Math.max(dels.length, adds.length);
      for (let j = 0; j < n; j++) {
        rows.push({
          kind: "line",
          key: `${file.path}:${hunk.oldStart}:s${seq++}`,
          file,
          hunk,
          left: dels[j],
          right: adds[j],
        });
      }
      continue;
    }
    if (line.kind === "add") {
      while (i < lines.length && lines[i]?.kind === "add") {
        rows.push({
          kind: "line",
          key: `${file.path}:${hunk.oldStart}:s${seq++}`,
          file,
          hunk,
          right: lines[i],
        });
        i++;
      }
      continue;
    }
    if (line.kind === "context") {
      rows.push({
        kind: "line",
        key: `${file.path}:${hunk.oldStart}:s${seq++}`,
        file,
        hunk,
        left: line,
        right: line,
      });
      i++;
      continue;
    }
    rows.push({ kind: "meta", key: `${file.path}:${hunk.oldStart}:m${seq++}`, text: line.text });
    i++;
  }
}

// fileDelta returns a file's additions and deletions.
export function fileDelta(file: DiffFile): { additions: number; deletions: number } {
  let additions = 0;
  let deletions = 0;
  for (const hunk of file.hunks) {
    for (const line of hunk.lines) {
      if (line.kind === "add") additions++;
      else if (line.kind === "del") deletions++;
    }
  }
  return { additions, deletions };
}

// maxLineChars is the widest text the current rows need, for horizontal
// scrolling. It ignores headers.
export function maxLineChars(rows: readonly Row[]): number {
  let max = 0;
  for (const row of rows) {
    if (row.kind !== "line") continue;
    for (const line of [row.left, row.right, row.line]) {
      if (line) max = Math.max(max, line.text.length);
    }
  }
  return max;
}

// statusLabel translates a DiffFile status for the page.
export function statusLabel(status: string): string {
  switch (status) {
    case "added":
      return "新增";
    case "deleted":
      return "删除";
    case "renamed":
      return "重命名";
    case "binary":
      return "二进制";
    case "mode":
      return "权限";
    default:
      return "修改";
  }
}
