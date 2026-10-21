// specs/13 刀⑤：压缩详情的**形态改造**（用户 2026-10-22 裁定：
// 「不要这几个卡片。直接在压缩的那个发丝线上触发展开，展开的不是以卡片的形式
// 显示。就跟模型消息那样向md那样显示就可以了，同时还要配折叠回发丝线状态的
// 按钮。让压缩的内容可展开可折叠。同时这个压缩内容需要在最顶层。不受那个L0
// 折叠层管理。属于永不折叠状态。跟模型最后汇报总结消息在一个层级的。」）。
//
// 这一份钉的是**界面上看得见的那几件事**（形态口径见《Front-end Design
// Guidelines》§二⑲）：
//  ① 压缩这一族是**一根发丝线**，不是居中窄卡片；
//  ② 展开体 = **模型正文形态**（md 渲染：列表项是真的 `<li>`、标题是真的
//     `<h2>`，不是一坨纯文本）；
//  ③ 点「查看详情」**就地展开**、再点**收起回发丝线状态**，且**不往转录里追加
//     条目**（旧实现是往转录里 push 一张新卡 ⇒ 点一次多一张、点几次堆几张）；
//  ④ 压缩线与展开体落在**轮容器之外**（顶层、永不折叠，与最终报告同级）。
//
// 挂载的是真宿主 `ChatTranscript.vue`（动作接线在它那儿，拆开单测就证明不了
// 「点了之后转录里没多出东西」）。数据用**事件投影**造（与生产同一条路）。
//
// ⚠ **摘要为什么以一段话开头**（不是直接 `## Objective`）：本仓测试环境是
// `happy-dom`，它的 `document.createNodeIterator` 首个 `nextNode()` 返回**根
// 节点**（违反 DOM 规范）⇒ DOMPurify 把 body 当成待消毒节点 ⇒ **输入里的第一
// 个元素连同标签一起被吃掉**（文本留下、标签没了）。浏览器（WebView2 /
// Chromium）按规范走 ⇒ 没有这回事，本仓真实渲染不受影响。
// ⇒ 判据要测**浏览器里的行为**，就不能把断言押在"第一个元素"上：摘要以一段
// 话开头，标题与列表项都不是第一个元素 ⇒ 断言照常成立。**别把这条判据改成
// "断言第一个元素"**（那测的是 happy-dom 的毛病，不是本仓的代码）。
import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises, mount, type VueWrapper } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import ChatTranscript from "../src/components/ChatTranscript.vue";
import { applyLocalePref, i18n } from "../src/i18n";
import { useChatStore } from "../src/stores/chat";

/** 摘要正文（md 源）：上游那套 SUMMARY_TEMPLATE 的形状（以一段话开头，
 *  见文件头关于 happy-dom 的那条注记）。 */
const SUMMARY =
  "前面一大段被压掉了。\n\n## Objective\n- 做完那件事\n\n## Key Points\n- 关键点一\n";

/** 同一个 pinia 实例给「造数据」与「挂组件」两边（两套实例 = 组件看见的是
 * 另一个空 store —— 挂载时传的就是这个实例）。 */
let pinia: ReturnType<typeof createPinia>;

beforeEach(() => {
  pinia = createPinia();
  setActivePinia(pinia);
  applyLocalePref("zh-CN");
  // jsdom 没有实现 scrollTo（ChatTranscript 跟随滚动会调它）——补一个空实现，
  // 不补的话挂载就炸，炸在跟本用例无关的地方。
  Element.prototype.scrollTo = () => {};
});

/** 造一场"轮中途压缩"：用户消息 → 工具 → **压缩** → 工具。
 *
 * 为什么要压缩**后面还有东西**：那样这根线落在轮容器**内层**（而不是尾段），
 * 正是「不受 L0 折叠层管理」要覆盖的那种位置。 */
function seedMidTurnCompaction() {
  const chat = useChatStore();
  chat.onEvent({ kind: "user_message", session_id: "s1", content: "干活", pos: 0 });
  chat.onEvent({
    kind: "tool_call",
    session_id: "s1",
    call_id: "c1",
    tool: "fs_read",
    args: null,
    side_effect: "read",
    plugin_version: "0.1.0",
  });
  chat.onEvent({
    kind: "context_compacted",
    session_id: "s1",
    from_tokens: 3000,
    to_tokens: 1000,
    strategy: "compact",
    summary: SUMMARY,
  });
  chat.onEvent({
    kind: "tool_call",
    session_id: "s1",
    call_id: "c2",
    tool: "fs_read",
    args: null,
    side_effect: "read",
    plugin_version: "0.1.0",
  });
  return chat;
}

const mountTranscript = () =>
  mount(ChatTranscript, {
    props: { composerBottom: 120 },
    global: { plugins: [pinia, i18n] },
  });

/** 找那一枚动作钮（按文案认——钮只有一枚，认 id 反而绕）。 */
const buttonByText = (w: VueWrapper, text: string) =>
  w.findAll("button").find((b) => b.text().trim() === text);

describe("压缩详情：发丝线上就地展开（§二⑲）", () => {
  it("压缩这一族是一根发丝线，不是居中窄卡片", async () => {
    seedMidTurnCompaction();
    const w = mountTranscript();
    await flushPromises();
    expect(w.find(".sys-hairline").exists(), "发丝线形态在").toBe(true);
    expect(
      w.find('[class*="max-w-"]').exists(),
      "居中窄卡片（max-w-[85%] 那一档）不许再出现",
    ).toBe(false);
  });

  it("点「查看详情」就地展开：md 正文 + 转录里不多出任何条目", async () => {
    const chat = seedMidTurnCompaction();
    const w = mountTranscript();
    await flushPromises();
    const before = chat.items.length;

    const btn = buttonByText(w, "查看详情");
    expect(btn, "线上那枚「查看详情」在").toBeTruthy();
    await btn!.trigger("click");
    await flushPromises();

    expect(w.find(".sys-body").exists(), "展开体在场").toBe(true);
    expect(w.find(".sys-body .md").exists(), "模型正文形态（.md 那一套）").toBe(true);
    expect(w.find(".sys-body h2").exists(), "md 渲染（标题是真 h2，不是纯文本）").toBe(true);
    expect(w.find(".sys-body h2").text()).toBe("Objective");
    expect(w.find(".sys-body li").exists(), "列表项是真 li").toBe(true);
    expect(w.find(".sys-body li").text()).toBe("做完那件事");
    expect(chat.items.length, "展开不往转录里追加条目").toBe(before);
  });

  it("再点一下收起回发丝线状态（同一枚钮切换两态）", async () => {
    const chat = seedMidTurnCompaction();
    const w = mountTranscript();
    await flushPromises();

    await buttonByText(w, "查看详情")!.trigger("click");
    await flushPromises();
    expect(w.find(".sys-body").exists()).toBe(true);
    // 展开态那一枚钮改说「收起」（入口唯一 = 线上那一个钮）
    expect(buttonByText(w, "收起"), "展开态改说「收起」").toBeTruthy();
    expect(buttonByText(w, "查看详情"), "收起态那枚不在了").toBeFalsy();

    await buttonByText(w, "收起")!.trigger("click");
    await flushPromises();
    expect(w.find(".sys-body").exists(), "收起后展开体退场").toBe(false);
    expect(buttonByText(w, "查看详情"), "回到收起态那枚钮").toBeTruthy();
    expect(chat.items.length, "来回切也不追加条目").toBe(chat.items.length);
  });

  it("压缩线在轮容器之外：轮容器收起时它仍在，展开体也还在", async () => {
    const chat = seedMidTurnCompaction();
    const w = mountTranscript();
    await flushPromises();

    // 顶层：不是轮容器的后代
    const line = w.find(".sys-hairline").element;
    expect(line.closest(".turn-box"), "压缩线不在轮容器里").toBeNull();

    // 轮容器（第 0 层）收起来 —— 压缩线与它的展开体不受这一层管
    await w.find(".turn-timer").trigger("click");
    await flushPromises();
    expect(w.find(".turn-box .sys-hairline").exists(), "容器里没有这根线").toBe(false);
    expect(w.find(".sys-hairline").exists(), "收起了它仍在（顶层）").toBe(true);

    await buttonByText(w, "查看详情")!.trigger("click");
    await flushPromises();
    expect(w.find(".sys-body").exists(), "展开体也在（顶层·永不折叠）").toBe(true);
    expect(chat.items.length, "全过程不多条目").toBe(chat.items.length);
  });
});
