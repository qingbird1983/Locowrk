// stores/chat/types.ts —— T22 刀一（2026-10-18）从 `stores/chat.ts` 搬来的**类型面**。
// 这里只有 `export type`：转录行的联合、用量两档（轮级 / 会话级）、附件预览条。
// 零逻辑、零 `this` ⇒ 搬它不改变任何行为；收益是"读投影的人不用先滚 158 行类型"。
// 消费面仍写 `from "../stores/chat"`（= 本目录的 index.ts）：index 把七个类型原样
// re-export 出去，41 处引用点一行没改（`moduleResolution: bundler` 把 ./chat 解析到 ./chat/index.ts）。
import type {
  AssetRefDto,
  ContentPartDto,
  FileChangeDto,
  ToolRowStatus,
  TurnState,
} from "../../types";

export type ToolItem = {
  id: number;
  kind: "tool";
  callId: string;
  tool: string;
  args: unknown;
  sideEffect: string;
  pluginVersion: string;
  /** 行相位（队列 Q12·C1）：`running` = 结果未回；`settled` = 结果已闭合
   * （成败仍读 `ok`，不在这里复制第二份真相）；`awaiting_approval` = 实时
   * 专属——审批请求一发出就点亮，**不落会话日志**，故重放永不出现在这里。 */
  status: ToolRowStatus;
  ok: boolean | null;
  output?: unknown;
  error?: string | null;
  /** 实时到达的时刻（Date.now()）；历史重放无此项（§3.7 时长只在实时
   * 轮次显示，重放不虚构时序）。 */
  ts?: number;
  doneTs?: number;
  /** G2 崩溃恢复：悬空未裁决（执行中进程退出，副作用未知）。行状态点
   * 走警示色 + 「结果未知」；裁决落日志后随 uncertain_resolved 复位。 */
  uncertain?: boolean;
};

export type ChatItem =
  | {
      id: number;
      kind: "user";
      text: string;
      ts?: number;
      /** 本轮收束时刻（v2.101，turn_done 的轮界事件投影）：第 0 层计时行
       * 重放读数（「已工作 N 分 N 秒」）的终点；缺省 = 旧日志无（不虚构）。
       * 调度轮没有用户消息，计时行本就无从起算，不受影响。 */
      endTs?: number;
      /** 源事件在会话日志里的位次（§3.34 编辑重发的定位键）。重放消息有；
       * 实时发送的没有（后端按「最后一条用户消息」解析）。 */
      pos?: number;
      /** 附件部件（批 D）：图片/PDF 引用；缺省 = 纯文本（旧消息渲染零变化）。 */
      parts?: ContentPartDto[];
      /** 本轮文件改动（队列 Q12·C1）：轮头容器数据，不是独立行——
       * 实时与重放同一来源（后端在 TurnEnd 处从插件上报聚合）。
       * 缺省 = 本轮没写文件（不发空数组）。 */
      files?: FileChangeDto[];
      /** 本轮收束状态（队列 Q12·第二刀 v2.205）：`agent://turn-header` 的权威读数。
       * 它取代的正是此前只能由前端 `stopRequested` 自算的「已停止」标记——那个标记
       * 不进会话日志，切会话 / 重启就没了。缺省 = 旧日志或旧后端没有这条事件 ⇒
       * 消费方按"不知道"处理，**不许拿 completed 兜底**。 */
      turnState?: TurnState;
      /** 本轮真正干活的毫秒数 = 墙钟区间 − Σ审批等待（等待源 = 日志里的
       * `ApprovalDecided` 两刻，v2.205 起才有）。缺省 = 数据层读不出 ⇒ 计时行
       * 回落到 `endTs - ts` 的墙钟读数；**不得折算成 0**（0 是"一秒没干活"的断言）。 */
      activeMs?: number;
    }
  | {
      id: number;
      kind: "assistant";
      text: string;
      ts?: number;
    }
  | ToolItem
  /** 思考行（§3.9，v4.11 记账时长）：实时 delta 起点记 ts、下一条过程项
   * 到达时收口 doneTs（`持续了 N 秒`）；历史重放整段无时戳 → 首句摘要。 */
  | {
      id: number;
      kind: "thinking";
      text: string;
      ts?: number;
      doneTs?: number;
    }
  | { id: number; kind: "error"; text: string }
  /** 审批决策记录行（§3.7 v4.11 裁定①）：无卡片底，折叠进过程组为
   * 平铺行（无组时回落兄弟节点）；已允许/已拒绝都是可审计事实。 */
  | { id: number; kind: "decision"; tool: string; allowed: boolean }
  /** G2 崩溃恢复 uncertain 决策卡（组外兄弟，同 error/notice 语义——
   * 折进过程组会被默认折叠藏住，决策面必须可见）。resolved 落定后
   * 收为单行记录（裁决 + 检查记录），工具行由合成 tool_result 闭合。 */
  | {
      id: number;
      kind: "uncertain";
      callId: string;
      tool: string;
      args: unknown;
      resolved?: { decision: "completed" | "not_executed" | "inspected"; note: string };
    }
  /** 会话内系统消息（SystemNotice 模式）：压缩通知 / 审批结果。 */
  | SystemItem;

/**
 * 系统消息是 UI 陈述而非会话数据：存 i18n key，渲染期翻译以跟随语言
 * 切换。key 落在 locales 的 notice.* 区；actions 是动作 id 列表（文案
 * notice.action.*），实际行为由对应批次接线（⑨ 撤销/详情）。
 */
export type SystemItem = {
  id: number;
  kind: "system";
  key: string;
  params?: Record<string, unknown>;
  actions?: string[];
  /** v2.143 发丝线形态：居中细字盖在一根发丝线上（非卡片）。 */
  tone?: "hairline";
  /** 同族标记（"compact"）：发起→收束事件就地改写同一条线。 */
  tag?: string;
  /** 展开体正文（**md 源**）：压缩线的摘要全文。
   *
   *  **唯一出处 = 这一格**（旧的 `noticeDetails` 映射已退役：它与 `items`
   *  平行存一份同一份正文 ⇒ 同一属性两个出处，删条目时必漏一处）。 */
  body?: string;
  /** 展开态（**这条线自己的状态**）：true = 展开体在场。
   *
   *  切换它**不改转录的条目集合**（旧实现是往转录里 push 一张新卡 ⇒ 点一次
   *  「查看详情」多一张、点几次堆几张 —— 用户 2026-10-22 点名的 bug）。 */
  expanded?: boolean;
};

/** 本轮 LLM 用量聚合（v2.58 §3.12）：llm_usage 事件按轮累计——用户消息
 * / 发送重置，重放同序重建。speed = 最后一步 output/duration（tokens/s）。 */
export type TurnUsage = {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  reasoning: number;
  total: number;
  speed: number | null;
  steps: number;
};

/** 会话累计用量（M0 观测，KV v2 §7）：llm_usage 事件全会话累计——与
 * turnUsage（轮级，user_message / send 重置）不同，只在 switchSession /
 * newSession 重置；实时与重放同源累计，切走切回重建。 */
export type SessionUsage = {
  input: number;
  output: number;
  cacheRead: number;
  cacheWrite: number;
  reasoning: number;
  total: number;
  steps: number;
};

/** 附件预览条条目（批 D §9.2，前端态不入会话日志）：入条即后台上传，
 * ready 后随消息作 parts 发出。localUrl = 本地 objectURL 预览（图片）；
 * error = 后端校验拒绝的文案（红框可移除重加）。 */
export type AttachmentItem = {
  key: number;
  name: string;
  mime: string;
  kind: "image" | "file";
  status: "uploading" | "ready" | "error";
  asset?: AssetRefDto;
  localUrl?: string;
  error?: string;
};
