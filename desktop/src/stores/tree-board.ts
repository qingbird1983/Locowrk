/**
 * 看板状态（队列 Q14 第三刀 · MB《Memory Tree Board Design v1》§2/§3）。
 *
 * 为什么是 store 而不是组件内 ref：这块界面被三个件同时消费（左栏 / 画布 / 汇聚
 * 面板），而它们之间的耦合不是「传 props」能说清的——逐层展开纪律决定汇聚面板
 * 出不出现、切会话要同时复位展开态与视口、点章节要把定位意图交给另一侧的组件。
 * 写进组件里就得把这些规则复制进三个壳，或拉一条事件线；两者都比一份 store 差。
 * store 也正好落进 Q11 起的测试面：纪律与行语义可以在不渲染的情况下钉住。
 *
 * ⚠ 只读纪律（MB §3.2 只读裁定）：本 store 一个写命令都不发——三路读取全走
 * `memory_tree_sessions / _chapters / _stream`；编辑面留在 `TreeSection.vue`，
 * 尾巴计数与视口记忆是 MB §6 的 T3（本刀不做，别在这里假装有）。
 */
import { defineStore } from "pinia";
import { memoryTreeChapters, memoryTreeSessions, memoryTreeStream } from "../lib/api";
import {
  buildGroups,
  clampScale,
  pendingStubs,
  streamUnlocked,
  ZOOM_STEP,
  type BoardGroup,
} from "../lib/board-model";
import type { StreamView, TreeChapterView, TreeSummaryView } from "../types";

/** 键位：一个会话 = (workspace, sessionId) 二元组（与 TreeSection 同一条尺）。 */
export const sessionKeyOf = (s: { workspace: string; sessionId: string }) =>
  `${s.workspace}\u0000${s.sessionId}`;

/** 一组会话流的读取态。loading / error 都按组各一份，切组互不牵连。 */
export interface StreamState {
  loading: boolean;
  error: string;
  view: StreamView | null;
}

const IDLE_STREAM: StreamState = { loading: false, error: "", view: null };

export const useTreeBoardStore = defineStore("treeBoard", {
  state: () => ({
    /** 会话切换器数据源（全项目，最近更新降序）；null = 还没拉过。 */
    sessions: null as TreeSummaryView[] | null,
    /** 当前项目（fsRoots 里的一项；`""` = 未归属 `_root` scope）。 */
    workspace: "",
    /** 当前会话 id。 */
    sessionId: "",
    /** 选中会话有没有投影（投影缺/坏 = false，画布走空态而不是空壳）。 */
    exists: false,
    /** 画布的组（总结组按收束序 + 一枚未归总结虚拟组）；装配只读 `chapterIds`。 */
    groups: [] as BoardGroup[],
    /** 整理中的占位章节（第 2 列末尾那枚「整理中…」桩，MB §2.2 第 2 条）。
     * 在 store 里挑出来而不是让组件调 `pendingStubs(view)`：原始清单不进 state，
     * 展开态与流缓存才有唯一一份真相。 */
    stubs: [] as TreeChapterView[],
    /** 展开态两把独立尺：总结卡（组键）与标题条（章节 id）。 */
    openGroups: [] as string[],
    openTitles: [] as string[],
    /** 每组的会话流读取态，键 = 组键。只增不收回：切回看过的组不重烧 IPC。 */
    streams: {} as Record<string, StreamState>,
    /** 世界层变换（§二⑯ 唯一变换件）。 */
    tx: 0,
    ty: 0,
    scale: 1,
    /** 拖拽中 = 整棵子树去过渡（pan 的零跟随要求）。 */
    dragging: false,
    /** 定位意图：点章节卡 → 面板滚到该小节并闪一次。tick 自增让连点两次也重放。 */
    locateChapterId: "",
    locateTick: 0,
    /** 换会话后置 true，画布量完包围盒消费掉（§3.3「自动适应一次」）。 */
    pendingFit: false,
    loading: false,
    error: "",
  }),
  getters: {
    /** 当前项目下有投影的会话（未归属档 = `""` scope）。 */
    projectSessions(state): TreeSummaryView[] {
      return (state.sessions ?? []).filter((s) => s.workspace === state.workspace);
    },
    /**
     * 汇聚面板可见的组（§3.2 逐层展开纪律**只在这里判一次**）：
     * 该轮总结已展开 ∧ 组内全部非占位章节的标题都已展开。收起总结的组即使
     * 标题展开态还留着，也不得露出会话流——两层条件少一个都不算满足。
     */
    panelGroups(state): BoardGroup[] {
      const open = new Set(state.openTitles);
      return state.groups.filter(
        (g) => state.openGroups.includes(g.key) && streamUnlocked(g, open),
      );
    },
  },
  actions: {
    /**
     * 拉投影清单，**并把清单交回调用方**。返回它不是为了好看：`this.sessions` 是
     * 在这里被改写 state 的，调用方紧接着再读属性时 TS 仍带着进入本函数之前的
     * 收窄结论（它不知道 state 被 await 里的赋值动过），拿回来的会是错的类型。
     * 返回同一份数 = 少一次回头读，也让"第一次进场"的装配只依赖局部量。
     */
    async openSessions(): Promise<TreeSummaryView[]> {
      if (this.loading) return this.sessions ?? [];
      this.loading = true;
      this.error = "";
      try {
        this.sessions = await memoryTreeSessions();
      } catch (e) {
        this.error = String(e);
        this.sessions = [];
      } finally {
        this.loading = false;
      }
      return this.sessions ?? [];
    },
    /** 选项目：进该项目最近更新的一条会话；项目下没树 = 只清选择。 */
    async selectProject(workspace: string) {
      this.workspace = workspace;
      const first = this.projectSessions[0];
      if (first) await this.selectSession(sessionKeyOf(first));
      else await this.clearSelection();
    },
    async selectSession(key: string) {
      const hit = (this.sessions ?? []).find((s) => sessionKeyOf(s) === key);
      if (!hit) return;
      this.workspace = hit.workspace;
      this.sessionId = hit.sessionId;
      // 换会话 = 换数据源：展开态、流缓存、视口、定位一次全清（§3.3 切会话重置），
      // 然后排一次「适应」。
      this.openGroups = [];
      this.openTitles = [];
      this.streams = {};
      this.tx = 0;
      this.ty = 0;
      this.scale = 1;
      this.locateChapterId = "";
      this.pendingFit = true;
      this.loading = true;
      this.error = "";
      try {
        const v = await memoryTreeChapters(hit.sessionId, hit.workspace);
        this.exists = v.exists;
        // 第 1 列（总结卡）永远全部可见 = 「树根可见」而不是「已展开」；标题列要
        // 点了总结才出来（§3.2 逐层展开纪律的第一层），所以这里保持上面那次清空。
        this.groups = v.exists ? buildGroups(v) : [];
        this.stubs = v.exists ? pendingStubs(v) : [];
      } catch (e) {
        this.error = String(e);
        this.exists = false;
        this.groups = [];
        this.stubs = [];
      } finally {
        this.loading = false;
      }
    },
    async clearSelection() {
      this.sessionId = "";
      this.exists = false;
      this.groups = [];
      this.stubs = [];
      this.openGroups = [];
      this.openTitles = [];
      this.streams = {};
      this.locateChapterId = "";
    },
    /**
     * 展开/收起某轮总结（第 1 列 → 第 2 列的那一层）。
     * 收起时**不**清它的标题展开态：重新点开恢复原状，比"每次点开都要重铺一遍"
     * 好；面板可见与否由 `BoardCanvas` 按「组开 ∧ 全部标题开」两条同时判，
     * 所以收起的组绝不会露出会话流（§3.2 逐层展开纪律）。
     */
    toggleGroup(groupKey: string) {
      this.openGroups = this.openGroups.includes(groupKey)
        ? this.openGroups.filter((k) => k !== groupKey)
        : [...this.openGroups, groupKey];
    },
    /**
     * 标题条展开/收起 = 决定第 3 列章节卡与第 4 列汇聚面板出现与否的那一层
     * （§3.2 逐层展开纪律）。读流不在这里发：由画布对「可见面板键清单」下注
     * （状态 → 效应只有一条通路，展开/收起/换组三条动作都汇到同一处判定）。
     */
    toggleTitle(chapterId: string) {
      this.openTitles = this.openTitles.includes(chapterId)
        ? this.openTitles.filter((id) => id !== chapterId)
        : [...this.openTitles, chapterId];
    },
    /** 读一整组会话流（一次 IPC 出原子视图；重复读走缓存）。 */
    async loadStream(groupKey: string) {
      const had = this.streams[groupKey];
      if (had?.view || had?.loading) return;
      this.streams = { ...this.streams, [groupKey]: { ...IDLE_STREAM, loading: true } };
      try {
        const v = await memoryTreeStream(this.sessionId, this.workspace, groupKey);
        this.streams = { ...this.streams, [groupKey]: { loading: false, error: "", view: v } };
      } catch (e) {
        this.streams = { ...this.streams, [groupKey]: { loading: false, error: String(e), view: null } };
      }
    },
    streamOf(groupKey: string): StreamState {
      return this.streams[groupKey] ?? IDLE_STREAM;
    },
    /** 点章节卡 = 定位（面板已汇聚才谈得上滚过去，未汇聚由界面给提示）。 */
    locate(chapterId: string) {
      this.locateChapterId = chapterId;
      this.locateTick += 1;
    },
    consumeFit() {
      this.pendingFit = false;
    },
    // ---- 视口三交互（§二⑯：pan / zoom / 定位） ----
    panBy(dx: number, dy: number) {
      this.tx += dx;
      this.ty += dy;
    },
    setDrag(on: boolean) {
      this.dragging = on;
    },
    /** 按钮档：直接改读数，不做数字滚动动画；范围钳进勾⑩。 */
    zoomBy(step: number) {
      this.scale = clampScale(this.scale + step);
    },
    zoomIn() {
      this.zoomBy(ZOOM_STEP);
    },
    zoomOut() {
      this.zoomBy(-ZOOM_STEP);
    },
    setScale(v: number) {
      this.scale = clampScale(v);
    },
    /** `1:1`（标准 §二⑯ 键盘路径 F 之外那一枚）：缩放回 100% **且世界原点回到
     * 视口左上角**——只改 scale 会让画面停在"放大一倍看右下角"的偏移上。 */
    zoomReset() {
      this.scale = 1;
      this.tx = 0;
      this.ty = 0;
    },
    setViewport(tx: number, ty: number, scale: number) {
      this.tx = tx;
      this.ty = ty;
      this.scale = clampScale(scale);
    },
  },
});

/** 缩放读数（控件条与会话浮层共用一把尺，不在两处各写一次百分号）。 */
export function zoomPercent(scale: number): string {
  return `${Math.round(scale * 100)}%`;
}
