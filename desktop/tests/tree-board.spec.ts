// Q14 第三刀：看板组件层（左栏 / 画布 / 汇聚面板）。
//
// 断言一律用 **i18n key 的当前档解析值**而不是中文串（同 `inject-section.spec.ts`
// 那条纪律：把文案抄进断言 = 测试替界面锁死一种语言，而它恰恰证明不了双语那一半），
// 关键条目再**同一条用例切到 en 跑一遍**。
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import BoardCanvas from "../src/components/memory/board/BoardCanvas.vue";
import BoardRail from "../src/components/memory/board/BoardRail.vue";
import StreamPanel from "../src/components/memory/board/StreamPanel.vue";
import { buildGroups } from "../src/lib/board-model";
import { i18n, applyLocalePref } from "../src/i18n";
import { useChatStore } from "../src/stores/chat";
import { useShellStore } from "../src/stores/shell";
import { useTreeBoardStore } from "../src/stores/tree-board";
import {
  CHAT_TITLE,
  GROUP_ERR,
  ONE_RUN_TWO_CHAPTERS,
  SESSION_ROWS,
  ROW_FIRST,
  SEC_B1_TITLE,
  SECTION_ERR,
  STREAM_ONE_SECTION_BROKEN,
  STREAM_TWO_SECTIONS,
  WS,
} from "./board-fixture";
import { onCommand, resetIpc } from "./tauri-stub";

const t = (key: string, params?: Record<string, unknown>) =>
  String(i18n.global.t(key, params as never));

const withGlobal = () => ({ global: { plugins: [i18n] } });

beforeEach(() => {
  setActivePinia(createPinia());
  applyLocalePref("zh-CN");
  resetIpc();
  onCommand("memory_tree_sessions", SESSION_ROWS);
  onCommand("memory_tree_chapters", ONE_RUN_TWO_CHAPTERS);
  onCommand("memory_tree_stream", STREAM_TWO_SECTIONS);
});

/** 把 store 摆到"已选中一条有树的会话"那一档（组件用例的前置只在这里写一次）。 */
function seededBoard() {
  const board = useTreeBoardStore();
  board.sessions = SESSION_ROWS;
  board.workspace = WS;
  board.sessionId = "s-1";
  board.exists = true;
  board.groups = buildGroups(ONE_RUN_TWO_CHAPTERS);
  return board;
}

describe("左栏（BoardRail）：会话行那一行读数", () => {
  it("行 = 标题 + 「N 总结 · N 章节」，两个数都取自清单侧的 summaryCount / chapterCount", async () => {
    const board = seededBoard(); // 当前项目 = WS ⇒ 列表只列该项目的会话
    const wrapper = mount(BoardRail, withGlobal());
    await flushPromises();
    expect(wrapper.text()).toContain(t("memory.boardCounts", { summaries: 1, chapters: 2 }));
    expect(wrapper.text()).not.toContain(t("memory.boardCounts", { summaries: 0, chapters: 3 }));

    board.workspace = ""; // 切到未归属档：第二条会话（0 总结 · 3 章节 + pending）进列表
    await wrapper.vm.$nextTick();
    const text = wrapper.text();
    expect(text).toContain(t("memory.boardCounts", { summaries: 0, chapters: 3 }));
    // 整理中徽章只挂在 pending 那一行（SESSION_ROWS 第二条 pending=true）
    expect(text).toContain(t("memory.boardOrganizing"));
    wrapper.unmount();
  });

  it("同一份数据切到 en 后读数跟着换词（双语可切 = 文案真的变了）", async () => {
    seededBoard();
    const wrapper = mount(BoardRail, withGlobal());
    await flushPromises();
    const zh = t("memory.boardCounts", { summaries: 1, chapters: 2 });
    applyLocalePref("en");
    await wrapper.vm.$nextTick();
    const en = t("memory.boardCounts", { summaries: 1, chapters: 2 });
    expect(en).not.toBe(zh);
    expect(wrapper.text()).toContain(en);
    expect(wrapper.text()).not.toContain(zh);
    applyLocalePref("zh-CN");
    wrapper.unmount();
  });

  it("「未归属」只在 `_root` scope 真有投影时进下拉（没树的项目列出来点了是空页）", async () => {
    // 判据 observable 走触发器回显：SettingsSelect 的触发器显当前值对应的 label，
    // 弹出面板 teleport 到 body，不靠它断言。
    const board = useTreeBoardStore();
    board.workspace = ""; // 当前就选在未归属档
    board.sessions = [SESSION_ROWS[0]!]; // 只有带项目的一条 ⇒ 该档不存在
    const wrapper = mount(BoardRail, withGlobal());
    await flushPromises();
    expect(wrapper.text()).not.toContain(t("memory.boardUnowned"));
    board.sessions = SESSION_ROWS; // 补上 workspace = "" 的那条
    await wrapper.vm.$nextTick();
    expect(wrapper.text()).toContain(t("memory.boardUnowned"));
    wrapper.unmount();
  });

  it("底部「返回主界面」= 出口与入口成对（点了回对话目的地）", async () => {
    seededBoard();
    const shell = useShellStore();
    const wrapper = mount(BoardRail, withGlobal());
    await flushPromises();
    const btn = wrapper.findAll("button").find((b) => b.text() === t("memory.boardBackToChat"));
    expect(btn).toBeDefined();
    await btn!.trigger("click");
    expect(shell.destination).toBe("chat");
    wrapper.unmount();
  });
});

describe("画布（BoardCanvas）：四列按逐层展开纪律一层层出来", () => {
  it("第 1 列 = 每轮总结一枚；点它才出第 2 列标题条", async () => {
    const board = seededBoard();
    const wrapper = mount(BoardCanvas, withGlobal());
    await flushPromises();
    expect(wrapper.findAll('[data-node^="s:"]')).toHaveLength(1);
    expect(wrapper.findAll('[data-node^="t:"]')).toHaveLength(0);
    await wrapper.find('[data-node^="s:"]').trigger("click");
    expect(board.openGroups).toEqual(["0:8"]);
    expect(wrapper.findAll('[data-node^="t:"]')).toHaveLength(2);
    wrapper.unmount();
  });

  it("点标题条才出第 3 列章节卡；再点 = 收回", async () => {
    const board = seededBoard();
    board.openGroups = ["0:8"];
    const wrapper = mount(BoardCanvas, withGlobal());
    await flushPromises();
    await wrapper.find('[data-node="t:b1"]').trigger("click");
    expect(board.openTitles).toEqual(["b1"]);
    expect(wrapper.findAll('[data-node^="c:"]')).toHaveLength(1);
    await wrapper.find('[data-node="t:b1"]').trigger("click");
    expect(board.openTitles).toEqual([]);
    expect(wrapper.findAll('[data-node^="c:"]')).toHaveLength(0);
    wrapper.unmount();
  });

  it("量不到位置就不渲染面板壳（宁可晚一帧，也不给一个 height:0 的空盒）", async () => {
    // happy-dom 的 getBoundingClientRect 恒给 0 ⇒ 测不出包围盒，正是这一档的现场复刻
    const board = seededBoard();
    board.openGroups = ["0:8"];
    board.openTitles = ["b1", "b2"];
    expect(board.panelGroups).toHaveLength(1); // 纪律满足
    const wrapper = mount(BoardCanvas, withGlobal());
    await flushPromises();
    expect(wrapper.findAll('[data-node^="p:"]')).toHaveLength(0);
    wrapper.unmount();
  });

  it("键盘路径：Esc 退出画布回主视图；方向键平移；+/- 缩放", async () => {
    const board = seededBoard();
    const shell = useShellStore();
    const wrapper = mount(BoardCanvas, withGlobal());
    await flushPromises();
    await wrapper.trigger("keydown", { key: "ArrowRight" });
    expect(board.tx).toBe(-44); // 两个网格步长（44px）
    await wrapper.trigger("keydown", { key: "+" });
    expect(board.scale).toBe(1.25);
    await wrapper.trigger("keydown", { key: "0" });
    expect(board.scale).toBe(1);
    expect(board.tx).toBe(0); // 1:1 要求世界原点回视口左上
    await wrapper.trigger("keydown", { key: "Escape" });
    expect(shell.destination).toBe("chat");
    wrapper.unmount();
  });

  it("选中会话但投影读不出来 = 空态说清是「读不出来」而不是「没有内容」", async () => {
    const board = seededBoard();
    board.exists = false;
    const wrapper = mount(BoardCanvas, withGlobal());
    await flushPromises();
    expect(wrapper.text()).toContain(t("memory.boardNoProjection"));
    expect(wrapper.text()).not.toContain(t("memory.boardEmptyProject"));
    wrapper.unmount();
  });

  it("该项目一条会话都没有 = 另一条空态文案（§二⑩ + 三态纪律：两者不共用）", async () => {
    const board = useTreeBoardStore();
    board.sessions = [];
    const wrapper = mount(BoardCanvas, withGlobal());
    await flushPromises();
    expect(wrapper.text()).toContain(t("memory.boardEmptyProject"));
    expect(wrapper.text()).not.toContain(t("memory.boardNoProjection"));
    wrapper.unmount();
  });

  it("控件条四枚顺序固定 = − / 读数 / ＋ / 适应，读数当前档文案", async () => {
    seededBoard();
    const wrapper = mount(BoardCanvas, withGlobal());
    await flushPromises();
    const labels = wrapper
      .findAll(".board-zoom__btn")
      .map((b) => b.attributes("aria-label"));
    expect(labels).toEqual([
      t("memory.boardZoomOut"),
      t("memory.boardZoomIn"),
      t("memory.boardZoomFit"),
    ]);
    expect(wrapper.find(".board-zoom__read").text()).toBe("100%");
    wrapper.unmount();
  });
});

describe("汇聚面板（StreamPanel）：一组一块，行复用既有形态", () => {
  const group = () => buildGroups(ONE_RUN_TWO_CHAPTERS)[0]!;

  it("小节头 = 章节标题 + 来源徽章 + 手动徽章；正文行来自同一切片口径", async () => {
    const wrapper = mount(StreamPanel, {
      ...withGlobal(),
      props: {
        group: group(),
        state: { loading: false, error: "", view: STREAM_TWO_SECTIONS },
        heightPx: 400,
      },
    });
    await flushPromises();
    const text = wrapper.text();
    expect(text).toContain(SEC_B1_TITLE);
    expect(text).toContain(t("memory.sourceMemoryManager"));
    expect(text).toContain(t("memory.sourceRule")); // 第二节是规则切分
    expect(text).toContain(t("memory.treeManual")); // edited=true 挂手动
    expect(text).toContain(t("memory.kindUser"));
    expect(text).toContain(ROW_FIRST);
    wrapper.unmount();
  });

  it("整组读不出来 → 一条组级错误；单节越界 → 只有那一节报错，其余照常", async () => {
    const broken = mount(StreamPanel, {
      ...withGlobal(),
      props: {
        group: group(),
        state: { loading: false, error: GROUP_ERR, view: null },
        heightPx: 300,
      },
    });
    await flushPromises();
    expect(broken.text()).toContain(t("memory.boardStreamFailed", { reason: GROUP_ERR }));
    broken.unmount();

    const partial = mount(StreamPanel, {
      ...withGlobal(),
      props: {
        group: group(),
        state: { loading: false, error: "", view: STREAM_ONE_SECTION_BROKEN },
        heightPx: 300,
      },
    });
    await flushPromises();
    expect(partial.text()).toContain(ROW_FIRST); // 好那一节照常
    expect(partial.text()).toContain(
      t("memory.boardSectionFailed", { reason: SECTION_ERR }),
    );
    expect(partial.text()).not.toContain(t("memory.boardStreamFailed", { reason: "" }));
    partial.unmount();
  });

  it("面板内没有可滚的行 ≠ 面板消失：空清单给「一个小节都没有」而不是空白盒", async () => {
    const wrapper = mount(StreamPanel, {
      ...withGlobal(),
      props: {
        group: group(),
        state: { loading: false, error: "", view: { sections: [], from: 0, to: 0, totalRows: 0 } },
        heightPx: 200,
      },
    });
    await flushPromises();
    expect(wrapper.text()).toContain(t("memory.boardStreamEmpty"));
    wrapper.unmount();
  });
});

describe("画布与会话元数据同源", () => {
  it("会话标题走 chat store 的旁车标题，取不到才回落 id 前缀", async () => {
    const chat = useChatStore();
    chat.sessions = [{ id: "s-1", title: CHAT_TITLE } as never];
    seededBoard();
    const wrapper = mount(BoardCanvas, withGlobal());
    await flushPromises();
    expect(wrapper.text()).toContain(CHAT_TITLE);
    wrapper.unmount();
  });
});
