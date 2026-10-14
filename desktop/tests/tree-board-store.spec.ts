// Q14 第三刀：看板 store（`src/stores/tree-board.ts`）。
//
// 挑这三族来钉，因为它们都是**画布替代路径之外唯一的规则持有处**：逐层展开的两层
// 条件、切会话必须一次清干净的东西、以及"什么时候才发流读取 IPC"。这三条一旦漂到
// 组件里，就会变成三个组件各判一次（§二⑯ 与本仓 §11 同条纪律）。
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@tauri-apps/api/core", async () => {
  const stub = await import("./tauri-stub");
  return { invoke: stub.invoke };
});

import { createPinia, setActivePinia } from "pinia";
import { sessionKeyOf, useTreeBoardStore } from "../src/stores/tree-board";
import {
  MEMBERSHIP_WINS_OVER_INTERVAL,
  ONE_RUN_TWO_CHAPTERS,
  SESSION_ROWS,
  STREAM_TWO_SECTIONS,
  WS,
} from "./board-fixture";
import { invokeCalls, onCommand, resetIpc } from "./tauri-stub";

const key1 = sessionKeyOf({ workspace: WS, sessionId: "s-1" });
const key2 = sessionKeyOf({ workspace: "", sessionId: "s-2" });

beforeEach(() => {
  setActivePinia(createPinia());
  resetIpc();
  onCommand("memory_tree_sessions", SESSION_ROWS);
});

describe("选会话：state 装配与「切会话一次清干净」", () => {
  it("选中后组、桩、投影在场标记一起落位，并排一次自动适应", async () => {
    onCommand("memory_tree_chapters", ONE_RUN_TWO_CHAPTERS);
    const board = useTreeBoardStore();
    await board.openSessions();
    await board.selectSession(key1);
    expect(board.exists).toBe(true);
    expect(board.groups.map((g) => g.key)).toEqual(["0:8"]);
    expect(board.pendingFit).toBe(true);
    expect(board.loading).toBe(false);
  });

  it("换会话 = 展开态 / 流缓存 / 视口 / 定位一次全清（§3.3 切会话重置）", async () => {
    onCommand("memory_tree_chapters", ONE_RUN_TWO_CHAPTERS);
    onCommand("memory_tree_stream", STREAM_TWO_SECTIONS);
    const board = useTreeBoardStore();
    await board.openSessions();
    await board.selectSession(key1);
    board.toggleGroup("0:8");
    board.toggleTitle("b1");
    board.toggleTitle("b2");
    await board.loadStream("0:8");
    board.setViewport(120, 80, 1.5);
    expect(board.streams["0:8"]?.view).not.toBeNull();

    onCommand("memory_tree_chapters", { ...MEMBERSHIP_WINS_OVER_INTERVAL, sessionId: "s-2" });
    await board.selectSession(key2);
    expect(board.openGroups).toEqual([]);
    expect(board.openTitles).toEqual([]);
    expect(board.streams).toEqual({}); // 缓存跟着数据源走，不跨会话留脏读
    expect(board.tx).toBe(0);
    expect(board.ty).toBe(0);
    expect(board.scale).toBe(1);
    expect(board.pendingFit).toBe(true);
  });

  it("投影读不出来 = exists:false，画布走空态而不是空壳", async () => {
    onCommand("memory_tree_chapters", {
      exists: false,
      workspace: WS,
      sessionId: "s-1",
      chapteredUntil: 0,
      pausedAuto: false,
      chapters: [],
      summaries: [],
    });
    const board = useTreeBoardStore();
    await board.openSessions();
    await board.selectSession(key1);
    expect(board.exists).toBe(false);
    expect(board.groups).toEqual([]);
    expect(board.stubs).toEqual([]);
  });

  it("选一个没有树的项目 = 清选择，不留上一个个人的展开态", async () => {
    onCommand("memory_tree_chapters", ONE_RUN_TWO_CHAPTERS);
    const board = useTreeBoardStore();
    await board.openSessions();
    await board.selectSession(key1);
    await board.selectProject("F:/proj/empty");
    expect(board.sessionId).toBe("");
    expect(board.groups).toEqual([]);
    expect(board.openTitles).toEqual([]);
  });
});

describe("panelGroups：汇聚面板的两层条件（§3.2 逐层展开）", () => {
  it("组没展开 ⇒ 即使全部标题展开也不许出现面板", async () => {
    onCommand("memory_tree_chapters", ONE_RUN_TWO_CHAPTERS);
    const board = useTreeBoardStore();
    await board.openSessions();
    await board.selectSession(key1);
    board.toggleTitle("b1");
    board.toggleTitle("b2");
    expect(board.panelGroups).toEqual([]); // 第 1 层没开：第 4 层绝不预显
    board.toggleGroup("0:8");
    expect(board.panelGroups.map((g) => g.key)).toEqual(["0:8"]);
  });

  it("组开了但只展开一个标题 ⇒ 还是不汇聚", async () => {
    onCommand("memory_tree_chapters", ONE_RUN_TWO_CHAPTERS);
    const board = useTreeBoardStore();
    await board.openSessions();
    await board.selectSession(key1);
    board.toggleGroup("0:8");
    board.toggleTitle("b1");
    expect(board.panelGroups).toEqual([]);
    board.toggleTitle("b2");
    expect(board.panelGroups).toHaveLength(1);
    board.toggleTitle("b1"); // 收起任一 ⇒ 立即不收
    expect(board.panelGroups).toEqual([]);
  });
});

describe("流读取：一次 IPC 一组，读过不再重烧", () => {
  it("同组两次请求只发一条命令，group 参数原样传后端", async () => {
    onCommand("memory_tree_stream", STREAM_TWO_SECTIONS);
    const board = useTreeBoardStore();
    await board.openSessions();
    board.workspace = WS;
    board.sessionId = "s-1";
    await board.loadStream("0:8");
    await board.loadStream("0:8");
    const sent = invokeCalls().filter((c) => c.cmd === "memory_tree_stream");
    expect(sent).toHaveLength(1);
    expect(sent[0]?.args).toEqual({ sessionId: "s-1", workspace: WS, group: "0:8" });
    expect(board.streamOf("0:8").view?.sections).toHaveLength(2);
  });

  it("读失败留错误文案在原位，不把「读不到」演成「没有内容」（§2.3 失败语义）", async () => {
    const board = useTreeBoardStore();
    await board.openSessions();
    board.workspace = WS;
    board.sessionId = "s-1";
    await board.loadStream("orphan"); // 没 onCommand ⇒ 替身 reject
    const st = board.streamOf("orphan");
    expect(st.view).toBeNull();
    expect(st.error).toContain("unstubbed command");
    expect(st.loading).toBe(false);
  });

  it("未归组的键就是后端认的那个字符串，前端不另造第三种形状", async () => {
    onCommand("memory_tree_stream", { sections: [], from: 0, to: 0, totalRows: 0 });
    const board = useTreeBoardStore();
    await board.openSessions();
    await board.loadStream("orphan");
    const sent = invokeCalls().filter((c) => c.cmd === "memory_tree_stream");
    expect(sent[0]?.args?.group).toBe("orphan");
  });
});
