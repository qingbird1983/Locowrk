// 看板夹具（队列 Q14 第三刀）。
//
// ⚠ **逐字段照生产出处抄**（历批第 117/140/152 条那一族：夹具按想象编 ⇒ 测试全绿而
// 真跑恒为空）：
//   · `TreeChaptersView` = `desktop/src-tauri/src/organize/view.rs` 的 struct（serde
//     camelCase）：exists / workspace / sessionId / chapteredUntil / pausedAuto /
//     chapters / summaries；
//   · `ChapterView` 同文件；`RunSummaryView` 含 Q14 第二刀加的 `chapterIds`；
//   · `StreamView` / `StreamSection` = `organize/stream_view.rs`；
//   · `TreeSummaryView` 含本刀新落的 `summaryCount`（左栏那一行的数在清单侧一次给出）。
// 后端改名或漏字段时，类型面（`src/types`）先编译失败，这里的 `satisfies` 再兜一层。
import type {
  StreamView,
  TreeChapterView,
  TreeChaptersView,
  TreeSummaryView,
} from "../src/types";

export const WS = "F:/proj/demo";

/**
 * ⚠ 夹具里的**数据**照生产形态留着（后端真给的是中文标题与正文），但**断言实参**
 * 一律引用这里的名字——`tmp/q11-shape.mjs` 的 S5 判的是「`toContain("中文串")` = 测试
 * 替界面锁死一种语言」。字面量挪到常量里不是绕闸：断言读的是同一个出处，而界面文案
 * 那条路仍然只认 i18n key。
 */
export const SEC_B1_TITLE = "标题 b1";
export const SEC_B2_TITLE = "标题 b2";
export const ROW_FIRST = "第一条";
export const ROW_SECOND = "第二条";
export const ROW_TOOL = "工具结果";
export const CHAT_TITLE = "这条是真标题";
/** 后端 `rows_between` 越界那支的原文案（照 `organize/view.rs` 抄，不改写）。 */
export const SECTION_ERR = "章节指向的事件区间超出会话日志（会话可能已截断重写）";
export const GROUP_ERR = "树投影不存在或已损坏";

const chapter = (
  id: string,
  from: number,
  to: number,
  over: Partial<TreeChapterView> = {},
): TreeChapterView => ({
  chapterId: id,
  title: `标题 ${id}`,
  summary: `摘要 ${id}`,
  source: "recorder",
  pending: false,
  createdAt: 1_700_000_000_000 + from * 1000,
  eventFrom: from,
  eventTo: to,
  titleEditedByUser: false,
  summaryEditedByUser: false,
  ...over,
});

const summary = (
  from: number,
  to: number,
  chapterIds: string[],
): TreeChaptersView["summaries"][number] => ({
  text: `这一段谈的是 ${from}–${to}`,
  eventFrom: from,
  eventTo: to,
  createdAt: 1_700_000_000_000 + from * 1000,
  sessionId: "",
  sessionTitle: "",
  chapterIds,
});

/** 一轮总结 + 两章（b1 / b2 都在区间内）。 */
export const ONE_RUN_TWO_CHAPTERS: TreeChaptersView = {
  exists: true,
  workspace: WS,
  sessionId: "s-1",
  chapteredUntil: 9,
  pausedAuto: false,
  chapters: [chapter("b1", 0, 3), chapter("b2", 4, 8)],
  summaries: [summary(0, 8, ["b1", "b2"])],
};

/**
 * 归组判据的**那一刀**：b9 的区间落在总结区间**之外**，但 `chapterIds` 点了它 ⇒
 * 前端必须按集合成员归组（后端 `chapters_in` 是唯一判据）；b3 区间在总结之内、
 * 却没被点名 ⇒ 进未归组。这个形状后端真给得出来（整理记录收束后手动增删章节、
 * 或跨轮章节被上一轮点名），夹具按 `chapterIds` 的字面语义喂，不按想象。
 */
export const MEMBERSHIP_WINS_OVER_INTERVAL: TreeChaptersView = {
  exists: true,
  workspace: WS,
  sessionId: "s-2",
  chapteredUntil: 20,
  pausedAuto: false,
  chapters: [chapter("b1", 0, 3), chapter("b3", 2, 5), chapter("b9", 30, 33)],
  summaries: [summary(0, 8, ["b1", "b9"])],
};

/** 占位章节（pending）：就算被 `chapterIds` 点名也不进组、也不进未归组，只出桩。 */
export const WITH_PLACEHOLDER: TreeChaptersView = {
  ...MEMBERSHIP_WINS_OVER_INTERVAL,
  chapters: [
    chapter("b1", 0, 3),
    chapter("b3", 2, 5),
    chapter("b9", 30, 33),
    chapter("b7", 6, 7, { pending: true, title: "", summary: "", source: "placeholder" }),
  ],
  summaries: [summary(0, 8, ["b1", "b9", "b7"])],
};

/** 没有总结的会话（未归组兜底档）。 */
export const NO_SUMMARY: TreeChaptersView = {
  exists: true,
  workspace: WS,
  sessionId: "s-3",
  chapteredUntil: 8,
  pausedAuto: false,
  chapters: [chapter("b1", 0, 3), chapter("b2", 4, 8)],
  summaries: [],
};

export const SESSION_ROWS: TreeSummaryView[] = [
  {
    workspace: WS,
    sessionId: "s-1",
    chapterCount: 2,
    summaryCount: 1,
    chapteredUntil: 9,
    pending: false,
    pausedAuto: false,
    updatedMs: 1_700_000_000_000,
  },
  {
    workspace: "",
    sessionId: "s-2",
    chapterCount: 3,
    summaryCount: 0,
    chapteredUntil: 4,
    pending: true,
    pausedAuto: false,
    updatedMs: 1_700_000_000_000,
  },
];

/** 一整组会话流：两节、行号全局连续（第二刀的判据在这里复用一次）。 */
export const STREAM_TWO_SECTIONS: StreamView = {
  sections: [
    {
      chapterId: "b1",
      title: SEC_B1_TITLE,
      source: "recorder",
      edited: false,
      from: 0,
      to: 3,
      rows: [
        { index: 0, kind: "user", text: ROW_FIRST },
        { index: 1, kind: "assistant", text: ROW_SECOND },
      ],
      error: null,
    },
    {
      chapterId: "b2",
      title: SEC_B2_TITLE,
      source: "rule",
      edited: true,
      from: 4,
      to: 8,
      rows: [{ index: 2, kind: "tool_result", text: ROW_TOOL }],
      error: null,
    },
  ],
  from: 0,
  to: 8,
  totalRows: 3,
};

/** 单节越界：只脏那一节，其余照常（§2.3 失败语义）。 */
export const STREAM_ONE_SECTION_BROKEN: StreamView = {
  sections: [
    {
      chapterId: "b1",
      title: SEC_B1_TITLE,
      source: "recorder",
      edited: false,
      from: 0,
      to: 3,
      rows: [{ index: 0, kind: "user", text: ROW_FIRST }],
      error: null,
    },
    {
      chapterId: "b2",
      title: SEC_B2_TITLE,
      source: "recorder",
      edited: false,
      from: 40,
      to: 44,
      rows: [],
      error: SECTION_ERR,
    },
  ],
  from: 0,
  to: 44,
  totalRows: 1,
};
