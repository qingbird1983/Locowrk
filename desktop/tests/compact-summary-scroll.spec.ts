// specs/13 刀⑨：压缩摘要展开体的**有界滚动**（用户 2026-10-22 原话：
// 「那个压缩摘要展开 可以做成展开后显示出10行，向下滚动上面的以压缩那根发丝
// 线为界切掉。意思就是压缩展开显示10行，可在压缩自己的容器里滚动。压缩那根
// 发丝线不会被摘要挤到上面看不见了。现在的展开摘要，想要折叠起来还有向上
// 滚动找到那根发丝才能再点到收起。」）。
//
// 这一份钉的是**展开体那三条样式口径**（形态口径见《Front-end Design
// Guidelines》§二⑲「有界滚动」，v2.6）：
//  ① 展开体**自己滚**：`overflow-y: auto` + `overscroll-behavior: contain`
//     （滚动孤岛 —— 读到底不连带转录）；
//  ② 展开体**封顶 10 行**：`max-height` 按 `.md` 自己的行高算（`10lh`）
//     ⇒ 发丝线与那枚「收起」钮不会被摘要顶出屏；
//  ③ **有界滚动 ≠ 折叠**：展开体恒展开（没有「展开摘要」这层动作），收起
//     仍只由线上那枚钮管。
//
// 断言读的是**计算样式**（`getComputedStyle`），不是源码字符串——两个前提
// 都写在这里，缺一个这条判据就只剩「字符串对得上」：
//  - `desktop/vitest.config.ts` 的 `css: true` ⇒ SFC 的 <style> 块真被编译
//    并注入文档（默认 false 时样式整块被丢弃）；
//  - 挂载要 `attachTo: document.body` ⇒ happy-dom 对**未连接**元素直接返回
//    空计算样式（`CSSComputedStyle.getComputedStyle` 里 `isConnected` 为假
//    就 return 空壳），VTU 默认不挂文档 ⇒ 不 attachTo 就读不到任何样式。
//
// ⚠ happy-dom **没有布局引擎**（不量行高、不算溢出）⇒ 这里钉的是**口径**
// （封顶值 + 滚动属性），**不是**「真的溢出了」。真的溢出成不成立由真机
// 截图核对（AGENTS.md §6.5：动了界面交付前自己在真实窗口跑一遍）。
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import ChatTranscript from "../src/components/ChatTranscript.vue";
import { applyLocalePref, i18n } from "../src/i18n";
import { useChatStore } from "../src/stores/chat";

/** 摘要正文（md 源）：**够长**——真机里要能撑破 10 行的封顶（happy-dom
 * 不排版，长短只影响真机那一眼）。以一段话开头（happy-dom 的
 * `document.createNodeIterator` 首个 `nextNode()` 返回根节点 ⇒ DOMPurify
 * 会把**第一个元素**连同标签吃掉 ⇒ 断言不能押在第一个元素上——见
 * `compact-summary-body.spec.ts` 文件头同款注记）。 */
const SUMMARY = [
  "前面一大段被压掉了，这里是压缩摘要。",
  "",
  "## Objective",
  "- 做完那件事",
  "",
  "## Key Points",
  "- 关键点一",
  "- 关键点二",
  "- 关键点三",
  "- 关键点四",
  "- 关键点五",
  "- 关键点六",
  "- 关键点七",
  "- 关键点八",
  "- 关键点九",
  "- 关键点十",
  "- 关键点十一",
  "- 关键点十二",
  "- 关键点十三",
  "- 关键点十四",
  "",
].join("\n");

let pinia: ReturnType<typeof createPinia>;
/** 挂出去的实例（`attachTo` 挂文档 ⇒ 用完得卸，否则下一条用例里 DOM 堆着
 * 上一条的东西）。 */
let mounted: VueWrapper | null = null;

beforeEach(() => {
  pinia = createPinia();
  setActivePinia(pinia);
  applyLocalePref("zh-CN");
  Element.prototype.scrollTo = () => {};
});

afterEach(() => {
  mounted?.unmount();
  mounted = null;
});

/** 造一场"轮中途压缩"（与 `compact-summary-body.spec.ts` 同一条数据路）。 */
function seedMidTurnCompaction() {
  const chat = useChatStore();
  chat.onEvent({ kind: "user_message", session_id: "s1", content: "干活", pos: 0 });
  chat.onEvent({
    kind: "context_compacted",
    session_id: "s1",
    from_tokens: 3000,
    to_tokens: 1000,
    strategy: "compact",
    summary: SUMMARY,
  });
  return chat;
}

const mountTranscript = () => {
  mounted = mount(ChatTranscript, {
    props: { composerBottom: 120 },
    global: { plugins: [pinia, i18n] },
    attachTo: document.body,
  });
  return mounted;
};

const buttonByText = (w: VueWrapper, text: string) =>
  w.findAll("button").find((b) => b.text().trim() === text);

/** 展开那枚钮（收起态说「查看详情」）。 */
async function expand(w: VueWrapper) {
  const btn = buttonByText(w, "查看详情");
  expect(btn, "线上那枚「查看详情」在").toBeTruthy();
  await btn!.trigger("click");
  await flushPromises();
}

/** 展开体那枚正文元素（封顶与滚动属性都住在它身上）。 */
const bodyMd = (w: VueWrapper) => {
  const md = w.find(".sys-body .md");
  expect(md.exists(), "展开体在").toBe(true);
  return md.element;
};

describe("压缩摘要展开体：有界滚动（§二⑲ v2.6）", () => {
  it("展开体自己滚：overflow-y auto + 滚到头不连带转录", async () => {
    seedMidTurnCompaction();
    const w = mountTranscript();
    await flushPromises();
    await expand(w);

    const cs = getComputedStyle(bodyMd(w));
    expect(cs.overflowY, "自己滚（不是靠转录整页滚）").toBe("auto");
    expect(cs.overscrollBehavior, "滚到头不连带转录（滚动孤岛）").toBe("contain");
  });

  it("展开体封顶 10 行（发丝线与「收起」钮不会被摘要顶出屏）", async () => {
    seedMidTurnCompaction();
    const w = mountTranscript();
    await flushPromises();
    await expand(w);

    const maxHeight = getComputedStyle(bodyMd(w)).maxHeight;
    // 口径 = 10 × `.md` 自己的行高（`lh` 单位随行高走，改字号不必改这里）
    expect(maxHeight, "封顶按行高算，且就是 10 行").toMatch(/^10lh$/);
    expect(maxHeight, "不许退回 none（那就回到「发丝线被挤走」的老毛病）").not.toBe(
      "none",
    );
  });

  it("线与那枚「收起」钮仍在线上，且排在展开体之前（不被顶走）", async () => {
    seedMidTurnCompaction();
    const w = mountTranscript();
    await flushPromises();
    await expand(w);

    // 展开态那一枚钮仍在线上（线 + 钮在展开体上方，不被摘要盖住）
    expect(buttonByText(w, "收起"), "展开态线上那枚钮还在").toBeTruthy();

    const line = w.find(".sys-hairline").element;
    const body = w.find(".sys-body").element;
    expect(
      line.compareDocumentPosition(body) & Node.DOCUMENT_POSITION_FOLLOWING,
      "发丝线排在展开体**之前**（线在上、摘要在下 ⇒ 摘要再长也顶不到线上面）",
    ).toBeTruthy();
  });

  it("有界滚动 ≠ 折叠：展开体恒展开，收起仍只由线上那枚钮管", async () => {
    const chat = seedMidTurnCompaction();
    const w = mountTranscript();
    await flushPromises();
    const before = chat.items.length;
    await expand(w);

    // 展开体里**没有**第二套展开/收起动作（不授权「展开摘要」这层动作）
    expect(w.findAll(".sys-body button").length, "展开体里没有自己的动作钮").toBe(0);

    // 收起 = 点线上那枚钮（不是滚到底、不是点第二枚钮）
    await buttonByText(w, "收起")!.trigger("click");
    await flushPromises();
    expect(w.find(".sys-body").exists(), "收起后展开体退场").toBe(false);
    expect(buttonByText(w, "查看详情"), "回到收起态那枚钮").toBeTruthy();
    expect(chat.items.length, "全过程不追加条目").toBe(before);
  });
});
