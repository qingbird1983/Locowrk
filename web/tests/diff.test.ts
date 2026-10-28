import assert from "node:assert/strict";
import { test } from "node:test";

import type { DiffDoc } from "../src/mygo";
import { buildRows, fileDelta, fileKey, hunkKey, maxLineChars } from "../src/diff/model";
import { languageFor, tokenize } from "../src/diff/tokens";

function doc(): DiffDoc {
  return {
    state: "ok",
    workspace: "repo",
    hidden: 0,
    skipped: 0,
    truncated: false,
    stats: { files: 1, additions: 2, deletions: 1, lines: 4 },
    files: [
      {
        path: "a.go",
        status: "modified",
        hunks: [
          {
            header: "@@ -1,2 +1,3 @@",
            oldStart: 1,
            oldCount: 2,
            newStart: 1,
            newCount: 3,
            lines: [
              { kind: "context", oldNo: 1, newNo: 1, text: "package main" },
              { kind: "del", oldNo: 2, text: "old" },
              { kind: "add", newNo: 2, text: "new" },
              { kind: "add", newNo: 3, text: "more" },
            ],
          },
        ],
      },
    ],
  };
}

test("unified rows keep every line", () => {
  const rows = buildRows(doc(), "unified", new Set());
  assert.equal(rows.length, 6);
  assert.equal(rows[0]?.kind, "file");
  assert.equal(rows[1]?.kind, "hunk");
  assert.equal(rows[2]?.line?.text, "package main");
  assert.equal(rows[4]?.line?.text, "new");
});

test("side rows pair deletions with additions and leave gaps", () => {
  const rows = buildRows(doc(), "side", new Set());
  const lines = rows.filter((r) => r.kind === "line");
  assert.equal(lines.length, 3);
  assert.equal(lines[1]?.left?.text, "old");
  assert.equal(lines[1]?.right?.text, "new");
  assert.equal(lines[2]?.left, undefined);
  assert.equal(lines[2]?.right?.text, "more");
});

test("folds hide a file or one hunk", () => {
  const d = doc();
  const all = buildRows(d, "unified", new Set());
  assert.equal(all.length, 6);
  const fileFolded = buildRows(d, "unified", new Set([fileKey(d.files[0]!)]));
  assert.equal(fileFolded.length, 1);
  const hunkFolded = buildRows(d, "unified", new Set([hunkKey(d.files[0]!, 0)]));
  assert.equal(hunkFolded.length, 2);
  assert.equal(fileDelta(d.files[0]!).additions, 2);
  assert.equal(fileDelta(d.files[0]!).deletions, 1);
  assert.equal(maxLineChars(all), "package main".length);
});

test("language hints and tokens", () => {
  assert.equal(languageFor("a/b.go"), "go");
  assert.equal(languageFor("a/b.unknown"), "plain");
  const tokens = tokenize('func main() { // hi', "go");
  assert.deepEqual(tokens[0], { text: "func", kind: "keyword" });
  assert.ok(tokens.some((t) => t.kind === "comment" && t.text === "// hi"));
  const str = tokenize('x := "a\\"b"', "go");
  assert.ok(str.some((t) => t.kind === "string"));
  assert.deepEqual(tokenize("x".repeat(501), "go"), [{ text: "x".repeat(501), kind: "plain" }]);
});
