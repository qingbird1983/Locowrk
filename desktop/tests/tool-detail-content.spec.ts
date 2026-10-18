// C3 展开体形态库（设计稿 §7.1 差异表 / §7.3 终端块）：`parseUnifiedDiff` 是
// 解析器，非平凡逻辑留这份最小可跑检查——它认错行号或把非 diff 当 diff 展示，
// 用户看到的就是假的「改了什么」。
import { describe, expect, it } from "vitest";
import { diffFromReplacement, parseUnifiedDiff } from "../src/lib/chat/diff";
import { toolDetail } from "../src/lib/tool-detail";

const tr = (key: string) => key;
const ctx = { roots: [] as string[], tr };
const flat = (rows: NonNullable<ReturnType<typeof parseUnifiedDiff>>) =>
  rows.map((r) => `${r.kind}:${r.oldNo ?? "-"}:${r.newNo ?? "-"}:${r.text}`);

describe("parseUnifiedDiff", () => {
  it("跳过文件头与 \\ No newline，hunk 头重置两侧行号", () => {
    const rows = parseUnifiedDiff(
      [
        "diff --git a/x.ts b/x.ts",
        "--- a/x.ts",
        "+++ b/x.ts",
        "@@ -3,2 +3,3 @@",
        " ctx",
        "-old",
        "+new",
        "+extra",
        "\\ No newline at end of file",
      ].join("\n"),
    );
    expect(rows && flat(rows)).toEqual([
      "hunk:-:-:@@ -3,2 +3,3 @@",
      "ctx:3:3:ctx",
      "del:4:-:old",
      "add:-:4:new",
      "add:-:5:extra",
    ]);
  });

  it("拿不到 hunk 头就返回 null（调用方降级 text，不把原文当代码）", () => {
    expect(parseUnifiedDiff("hello\n+not a diff")).toBeNull();
    expect(parseUnifiedDiff("")).toBeNull();
  });
});

describe("diffFromReplacement", () => {
  it("无上下文构造：先删后加；行区间已知时旧行号照实推进", () => {
    expect(flat(diffFromReplacement("a\nb", "c", 7))).toEqual([
      "del:7:-:a",
      "del:8:-:b",
      "add:-:-:c",
    ]);
    expect(flat(diffFromReplacement("", "c", null))).toEqual(["add:-:-:c"]);
  });
});

describe("toolDetail 的内容块（§7.1 / §7.3）", () => {
  it("edit_diff 的 args.patch 解析成 diff 块，属性块排在它前面", () => {
    const d = toolDetail(
      "edit_diff",
      { path: "/w/a.ts", patch: "@@ -1 +1 @@\n-x\n+y" },
      { path: "/w/a.ts" },
      ctx,
    );
    expect(d.blocks.map((b) => b.kind)).toEqual(["props", "diff"]);
  });

  it("edit_lines 只有行区间与新内容时退化构造（不编造上下文行）", () => {
    const d = toolDetail(
      "edit_lines",
      { path: "/w/a.ts", start_line: 4, end_line: 5, content: "new" },
      { path: "/w/a.ts" },
      ctx,
    );
    const diff = d.blocks.find((b) => b.kind === "diff");
    expect(diff && diff.kind === "diff" ? flat(diff.rows) : null).toEqual(["add:-:-:new"]);
  });

  it("fs_write 整文写入没有差异可言，改给代码块", () => {
    const d = toolDetail("fs_write", { path: "/w/a.ts", content: "hi" }, { path: "/w/a.ts" }, ctx);
    expect(d.blocks.map((b) => b.kind)).toEqual(["props", "code"]);
  });

  it("写入的备份行并进同一个属性块（两个 dl 会让值列错位）", () => {
    const d = toolDetail(
      "fs_write",
      { path: "/w/a.ts", content: "hi" },
      { path: "/w/a.ts", bytes: 2, edits: 1, existed: true, backup: "bak/a.ts" },
      ctx,
    );
    const props = d.blocks.filter((b) => b.kind === "props");
    expect(props.length).toBe(1);
    const labels = props[0].kind === "props" ? props[0].rows.map((r) => r.label) : [];
    expect(labels).toContain("tool.label.backup");
  });

  it("shell 展开是终端块，命令与 stdout 合并", () => {
    const d = toolDetail("shell", { command: "ls" }, { exit_code: 0, stdout: "a.ts\n" }, ctx);
    const term = d.blocks.find((b) => b.kind === "term");
    expect(term && term.kind === "term" ? { command: term.command, output: term.output } : null).toEqual({
      command: "ls",
      output: "a.ts",
    });
  });
});
