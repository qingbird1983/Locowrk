// specs/12 挂账（刀⑦ 结清）：**传输重试**时，流式缓冲的重置要连**思考行**一起清。
//
// 判据的由来：改前 `resetAssistant` 只清末尾的 **assistant 气泡**。可推理的模型
// （DeepSeek / Qwen / GLM 那一家）在**正文还没到**之前先吐推理 ⇒ 断流时末尾那行
// 是**思考行** ⇒ 改前一个字都不清 ⇒ 失败那趟的推理留在行里，而重试轮的推理是
// **追加**到同一行的（`appendThinking`）⇒ 用户看到一段**没发生过的推理**与新一趟
// 叠在一起（"我刚才想的是 A，结果我想的是 B"）。
//
// 这一条盯的是**清空**这件事本身；"重试轮的 delta 重建哪一个行"是既有形状
// （assistant 气泡也是清空后另起一行），不在这一刀改。
import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useChatStore } from "../src/stores/chat";

const S = "s1";

/** 思考行的正文（按出现顺序）。 */
const thinkingTexts = (chat: ReturnType<typeof useChatStore>): string[] =>
  chat.items
    .filter((i) => i.kind === "thinking")
    .map((i) => (i as { text: string }).text);

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("传输重试：流式缓冲重置", () => {
  it("末尾是思考行时也要清空（失败那趟的推理不许留在行里）", () => {
    const chat = useChatStore();
    chat.onEvent({ kind: "thinking", session_id: S, text: "失败那趟的推理" });
    chat.onEvent({ kind: "llm_retry", session_id: S, attempt: 1, delay_ms: 500 });
    expect(thinkingTexts(chat)).toEqual([""]);
  });

  it("重试轮的推理不与失败那趟叠在一起", () => {
    const chat = useChatStore();
    chat.onEvent({ kind: "thinking", session_id: S, text: "失败那趟的推理" });
    chat.onEvent({ kind: "llm_retry", session_id: S, attempt: 1, delay_ms: 500 });
    chat.onEvent({ kind: "thinking", session_id: S, text: "重试那趟的推理" });
    expect(thinkingTexts(chat).join("|")).not.toContain("失败那趟");
    expect(thinkingTexts(chat).join("|")).toContain("重试那趟");
  });

  it("末尾是 assistant 气泡时照旧清空（既有语义不许被改坏）", () => {
    const chat = useChatStore();
    chat.onEvent({ kind: "chunk", session_id: S, delta: "半句" });
    chat.onEvent({ kind: "llm_retry", session_id: S, attempt: 1, delay_ms: 500 });
    const bubble = chat.items.find((i) => i.kind === "assistant");
    expect((bubble as { text: string } | undefined)?.text).toBe("");
  });
});
