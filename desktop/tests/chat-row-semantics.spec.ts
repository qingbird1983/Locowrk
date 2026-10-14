// Q11 第二条：store 层的**行语义**（`src/stores/chat.ts`）。
// 为什么挑这一族而不是别处：Q12（CF 稿 §10 · C1）要动的就是 `items` 的 append-or-create
// 与「实时 / 重放同形」这两条不变式，而它们今天**零机器证明**——只靠真机走查看气泡对不对。
// 这一条用例不是给现状打分，是给 Q12 铺第一层重放回归的落地面（队列 §3 第 3 条「解锁数」
// 里 Q11 存在的全部理由）。
import { beforeEach, describe, expect, it } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { useChatStore } from "../src/stores/chat";

beforeEach(() => {
  // pinia 原生档：`@pinia/testing` 的 peer 要 `pinia >= 4.0.2` 而本仓是 `^3.0.0`
  // （实测 pnpm view），引它 = 制造版本冲突 ⇒ 用 `createPinia + setActivePinia`。
  setActivePinia(createPinia());
});

describe("chat store 的行语义（append-or-create / 实时与重放同形）", () => {
  it("相邻的同 kind delta 并进同一行，不各造一条气泡", () => {
    const chat = useChatStore();
    chat.appendAssistant("你好");
    chat.appendAssistant("，世界");
    expect(chat.items).toHaveLength(1);
    expect(chat.items[0]?.kind).toBe("assistant");
    expect((chat.items[0] as { text: string }).text).toBe("你好，世界");
  });

  it("kind 一变就新建一行，并把上一条思考行收口（补 doneTs = 「持续了 N 秒」的来源）", () => {
    const chat = useChatStore();
    chat.appendThinking("先想一下");
    chat.appendThinking("再想一下"); // 同 kind：并
    chat.appendAssistant("结论");
    expect(chat.items).toHaveLength(2);
    const thinking = chat.items[0] as { kind: string; text: string; doneTs?: number };
    expect(thinking.kind).toBe("thinking");
    expect(thinking.text).toBe("先想一下再想一下");
    expect(thinking.doneTs).toBeTypeOf("number"); // 被下一条过程项收口
  });

  it("空 delta 不建行（appendThinking 的 `!text` 早退支）", () => {
    const chat = useChatStore();
    chat.appendThinking("");
    expect(chat.items).toHaveLength(0);
  });

  it("重放期新建的行**不带 ts**，实时期带——同一映射两种时间戳口径", () => {
    const chat = useChatStore();
    chat.replaying = true;
    chat.appendAssistant("历史正文");
    const replayed = chat.items[0] as { ts?: number };
    expect("ts" in replayed).toBe(false);

    // ⚠ 这里必须**换一个 kind** 再断言，不能接着 appendAssistant：实现的分行口径是
    //   「末项同 kind 就并」而不是「按 replaying 边界分行」（本批判据自纠第 142 条——
    //   第一版这里写的就是第二次 appendAssistant，结果并进了同一行、items[1] undefined）。
    chat.replaying = false;
    chat.appendThinking("新一轮的思考");
    const live = chat.items[1] as { ts?: number };
    expect(live.ts).toBeTypeOf("number");
  });

  it("行 id 单调递增且不重复（模块级计数器的口径按相对量判，不写死数字）", () => {
    const chat = useChatStore();
    chat.appendAssistant("a");
    chat.appendThinking("b");
    chat.appendAssistant("c");
    const ids = chat.items.map((i) => (i as { id: number }).id);
    expect(new Set(ids).size).toBe(ids.length);
    expect(ids[1]! > ids[0]!).toBe(true);
    expect(ids[2]! > ids[1]!).toBe(true);
  });
});
