// S2 刀⑦（specs/09 AC-S2-11）：`/整理` 改语义 = 当场跑一次**主层压缩**，旧名不再被拦。
//
// 形状照 `chat-projection.spec.ts`（store 级直调 action，Q11 那条"投影不可单测"的旧论断
// 已不成立），IPC 走 `tauri-stub`；断言一律认 **i18n key 本身**而不是中文串（同
// `context-section.spec.ts` 的口径）。
//
// 各条抓一层，彼此不重叠：
//  ① 命令被拦 ⇒ 只发 `compact_now`、**不发 `agent_chat`**，也不建用户气泡——元操作
//     不进对话流这条是从旧 `/整理` 继承来的，改语义不许把它改丢；
//  ② 压成时那根线**不收走**、也不改写成人话：改成「已压缩」那句中间态，等
//     `agent://context-compacted` 那条事件来把它收成结果线（省了多少由那条
//     说；这里再留一句「压缩完成」就是两句同义）；
//  ③ 没压成 ⇒ 在途线**就地改写**成对应判据那一条（一根线从头到尾，不是两根）；
//  ④ 后端给了认不出的判据名 ⇒ 兜底那条并把名字**原样带出**（绝不悄悄说成已压缩）；
//  ⑤ `/整理` 不再被拦 = 改名的负对照：它现在就是一条普通消息；
//  ⑥ 发起抛错 ⇒ 在途线收走 + 原文系统条（不留一根悬着的「正在压缩」）。
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import { createPinia, setActivePinia } from "pinia";

import { useChatStore } from "../src/stores/chat";
import { applyLocalePref } from "../src/i18n";
import { invoke, invokeCalls, onCommand, resetIpc } from "./tauri-stub";

const SYS = "system";

/** 只取系统条的 key（顺序 = 落列顺序）。认 key 不认文案：文案改一个字不该红这里。 */
const sysKeys = (chat: ReturnType<typeof useChatStore>) =>
  chat.items.filter((i) => i.kind === SYS).map((i) => (i as { key?: string }).key);

const fresh = () => {
  const chat = useChatStore();
  chat.sessionId = "s-1";
  return chat;
};

beforeEach(() => {
  setActivePinia(createPinia());
  applyLocalePref("zh-CN");
  resetIpc();
});

describe("`/压缩` 这条元操作", () => {
  it("中文名与英文别名都只发 compact_now，不发 agent_chat、不建用户气泡", async () => {
    onCommand("compact_now", { compacted: true, reason: null });
    for (const typed of ["/压缩", "/compact", "  /压缩  "]) {
      const chat = fresh();
      await chat.submit(typed);
      const cmds = invokeCalls().map((c) => c.cmd);
      expect(cmds, `${typed} 应当只发一次压缩`).toEqual(["compact_now"]);
      expect(chat.items.some((i) => i.kind === "user")).toBe(false);
      resetIpc();
    }
  });

  it("压成时那根线不收走也不多说一句：改成「已压缩」那句中间态", async () => {
    onCommand("compact_now", { compacted: true, reason: null });
    const chat = fresh();
    await chat.submit("/压缩");
    expect(sysKeys(chat)).toEqual(["notice.compactDone"]);
    // 整场一条都不多：省了多少由 `agent://context-compacted` 那条事件负责（另有
    // 用例钉那条投影），这里若顺手落一句「压缩完成」就是两句同义。
    expect(chat.items).toHaveLength(1);
  });

  it("没压成 ⇒ 同一根线就地改写成那条判据的文案（一条线，不是两根）", async () => {
    // 判据名跟着上游换了一整套（specs/13 契约 C3）：旧的 `too-few-rounds` 退役，
    // "没有可剪的一段" 在上游叫 `nothing-to-compact`。
    onCommand("compact_now", { compacted: false, reason: "nothing-to-compact" });
    const chat = fresh();
    await chat.submit("/压缩");
    expect(sysKeys(chat)).toEqual(["notice.compactNothingToCompact"]);
    const line = chat.items.find((i) => i.kind === SYS) as { tone?: string; tag?: string };
    expect(line.tone, "收束后仍是那根发丝线（v2.143 的形态）").toBe("hairline");
    expect(line.tag).toBe("compact");
  });

  it("认不出的判据名走兜底那条，并把名字原样带出（不许说成已压缩）", async () => {
    onCommand("compact_now", { compacted: false, reason: "brand-new-reason" });
    const chat = fresh();
    await chat.submit("/压缩");
    const line = chat.items.find((i) => i.kind === SYS) as {
      key?: string;
      params?: Record<string, unknown>;
    };
    expect(line.key).toBe("notice.compactUnknown");
    expect(line.params?.reason).toBe("brand-new-reason");
  });

  it("旧名 `/整理` 不再被拦：它现在就是一条普通消息（改名的负对照）", async () => {
    onCommand("agent_chat", 1);
    const chat = fresh();
    await chat.submit("/整理");
    const cmds = invokeCalls().map((c) => c.cmd);
    expect(cmds, "旧名不许继续被当成元操作吞掉").toContain("agent_chat");
    expect(cmds).not.toContain("compact_now");
    expect(chat.items.some((i) => i.kind === "user")).toBe(true);
  });

  it("发起抛错 ⇒ 在途线收走 + 原文系统条，不留一根悬着的「正在压缩」", async () => {
    // 故意**不**预置 compact_now：stub 按"没预置 = reject"的口径把它当发起失败
    const chat = fresh();
    await chat.submit("/压缩");
    expect(sysKeys(chat)).toEqual(["__raw__"]);
  });

  it("手动这一趟途中流回来的摘要正文 ⇒ 挂到在途那根线上（事件发了界面要动）", async () => {
    // 后端那条 `delta` 相位**不分手动自动**（上游 `Compaction.Delta` 同款）。
    // 这一条盯的是"手动那根线也接得到"：只认自动那根的话，事件发了、界面不动，
    // 而事件照发——失败是静默的。
    let resolveCmd: (v: unknown) => void = () => {};
    invoke.mockImplementationOnce(
      () =>
        new Promise((res) => {
          resolveCmd = res as (v: unknown) => void;
        }),
    );
    const chat = fresh();
    const pending = chat.submit("/压缩");
    const line = chat.items.find((i) => i.kind === SYS) as {
      id: number;
      key?: string;
      actions?: string[];
      body?: string;
    };
    expect(line.key, "命令还没回来 ⇒ 在途线在").toBe("notice.compactRunning");
    expect(line.actions, "在途那根线带一个「查看详情」").toEqual(["details"]);

    chat.onEvent({
      kind: "compact_status",
      session_id: "s-1",
      phase: "delta",
      text: "## Objective\n- 做完那件事\n",
    });
    expect(line.body, "流回来的正文挂在在途那根线上").toBe("## Objective\n- 做完那件事\n");
    // delta 不另起线：转录里仍是同一根（形态只有一种）。
    expect(chat.items.filter((i) => i.kind === SYS)).toHaveLength(1);

    resolveCmd({ compacted: true, reason: null });
    await pending;
  });
});
