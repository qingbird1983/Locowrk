// Seed a session file with many messages for the Phase 3 long-session check.
//
// Usage: node scripts/seed-long-session.mjs <home> <cwd> [count]
// Prints the session file path and its session id.
import { mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { randomBytes } from "node:crypto";

const [home, cwdArg, countArg] = process.argv.slice(2);
if (!home || !cwdArg) {
  console.error("usage: node scripts/seed-long-session.mjs <home> <cwd> [count]");
  process.exit(2);
}
const count = Number(countArg ?? 5000);
const cwd = resolve(cwdArg);
const escaped = cwd.replace(/^[/\\]/, "").replace(/[/\\:]/g, "-");
const dir = join(home, ".pi", "agent", "sessions", `--${escaped}--`);
mkdirSync(dir, { recursive: true });

const now = new Date();
const stamp = now.toISOString().replace(/[:.]/g, "-");
const id = randomBytes(16).toString("hex");
const file = join(dir, `${stamp}_${id}.jsonl`);

const lines = [];
const header = { cwd, id, timestamp: now.toISOString(), type: "session", version: 3 };
lines.push(JSON.stringify(header));

const entryId = () => randomBytes(4).toString("hex");
const model = entryId();
lines.push(JSON.stringify({ id: model, modelId: "faux-1", parentId: null, provider: "faux", timestamp: now.toISOString(), type: "model_change" }));
const thinking = entryId();
lines.push(JSON.stringify({ id: thinking, parentId: model, thinkingLevel: "off", timestamp: now.toISOString(), type: "thinking_level_change" }));

let parent = thinking;
let ms = Date.now();
for (let i = 0; i < count; i++) {
  const role = i % 2 === 0 ? "user" : "assistant";
  const text = role === "user" ? `长会话问题 ${i}` : `长会话回答 ${i}${i === count - 1 ? " long marker" : ""}`;
  const message =
    role === "user"
      ? { role, content: [{ type: "text", text }], timestamp: ms }
      : {
          role,
          content: [{ type: "text", text }],
          api: "faux:seed",
          provider: "faux",
          model: "faux-1",
          usage: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, totalTokens: 0, cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 } },
          stopReason: "stop",
          timestamp: ms,
          thinkingLevel: "off",
        };
  const entry = entryId();
  lines.push(JSON.stringify({ id: entry, message, parentId: parent, timestamp: new Date(ms).toISOString(), type: "message" }));
  parent = entry;
  ms += 1;
}

writeFileSync(file, lines.join("\n") + "\n");
console.log(JSON.stringify({ file, id, messages: count }));
