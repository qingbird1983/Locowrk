// stores/chat/getters.ts —— chat store 的三个派生读数（T22 刀二搬出）。
// 它们本来就只吃 state（原实现写的就是 `cacheStats(state)` 这种参数形，没用 `this`），
// 所以搬出来零阻力；类型标注成 ChatState 是**收紧**不是改写（原来 state 参数的类型由
// pinia 现场推断，现在由 state.ts 那一份声明提供）。
import type { AssetRefDto } from "../../types";
import { toolImageAssets } from "../../lib/tool-summary";
import type { ChatState } from "./state";

export const chatGetters = {
  /** M0 观测（KV v2 §7）：会话累计缓存统计。命中率 = cacheRead /
   * input（**分母不含 cacheWrite**——纪律 7，加了会把命中率压低）。
   * v2.137：节省估算随两档单价设置退役（真实价格模型各异，不发明
   * 全局估算）。cacheRead 为 0 = 供应商未报告缓存字段或全 miss →
   * null（行不渲染，宁缺勿假）。 */
  cacheStats(state: ChatState): {
    hitRate: number;
    input: number;
    cacheRead: number;
    cacheWrite: number;
    total: number;
    steps: number;
  } | null {
    const u = state.sessionUsage;
    if (!u || u.input <= 0 || u.cacheRead <= 0) return null;
    return {
      hitRate: Math.round((u.cacheRead / u.input) * 100),
      input: u.input,
      cacheRead: u.cacheRead,
      cacheWrite: u.cacheWrite,
      total: u.total,
      steps: u.steps,
    };
  },
  /** 当前归属——工作区 chip 与欢迎页问候的**唯一同源真源**（v2.61 用户
   * 裁定「把欢迎语项目名跟对话框下面的项目名关联」）：会话态 = 活动会话
   * 旁车 workspace；首页空态 = 挂起归属。此前问候另有「最近活动会话 /
   * 第一个 fsRoot」回落，chip 显示「选择文件夹」时问候仍挂旧项目名——
   * 回落退役，两边永远同变。null = 未选项目（chip「选择文件夹」+ 问候
   * 不带名）。 */
  currentWorkspace(state: ChatState): string | null {
    const inSession =
      state.sessions.some((s) => s.id === state.sessionId) || state.running;
    if (inSession) {
      return (
        state.sessions.find((s) => s.id === state.sessionId)?.workspace ?? null
      );
    }
    return state.pendingWorkspace;
  },
  /** 转录图片清单（批 E 查看器）：用户 parts 图片 + 工具产物图片
   * （output.asset/assets，mime image/*）按转录序——查看器左右切换的
   * 列表真源。打开时求值一次（打开后新增图不回填游标，关闭重开即可）。 */
  viewerImages(state: ChatState): AssetRefDto[] {
    const out: AssetRefDto[] = [];
    for (const it of state.items) {
      if (it.kind === "user" && it.parts) {
        for (const p of it.parts) if (p.type === "image") out.push(p.asset);
      } else if (it.kind === "tool" && it.status === "settled" && it.output) {
        out.push(...toolImageAssets(it.output));
      }
    }
    return out;
  },
};
