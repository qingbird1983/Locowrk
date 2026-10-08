<script setup lang="ts">
/**
 * 消息流渲染核心（v2.104 用户裁定：子代理工作过程的分层折叠消息流 =
 * 主对话整个流程原样复用）：从 ChatTranscript 提取的共享渲染体——连续
 * 过程项合成动作块（§3.7 v4.11，决策记录行不渲染）、任务容器（v2.99
 * 「第 0 层」：计时行常驻头部 + 开合覆盖，收束收缩为计时行）、组相位机
 * （L3 live / L2 replied / L1 done）、复制钮判据（只在用户消息与最终
 * 汇报之下）、待决审批索引。父层提供滚动容器：主对话 = ChatTranscript
 * （跟随滚动/回到底部/列宽柄/编辑重发），子代理转录页签 =
 * SubagentTranscriptTab（只读快照）——行为差异全部走 props/emits：
 * - running：最后一轮执行中态（相位机 live 判据）；页签快照不传 =
 *   false，全落 done，与主对话历史重放同形；
 * - stopRequested：「已停止」只标最新过程组；页签恒 false；
 * - editableIds：可编辑用户气泡集合（§3.34）；页签只读不传；
 * - noticeAction：系统条动作上抛（撤销压缩/查看详情等）；页签不接。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { ChatItem, SystemItem, ToolItem } from "../../stores/chat";
import { useApprovalsStore } from "../../stores/approvals";
import { useSettingsStore } from "../../stores/settings";
import { toolAction } from "../../lib/tool-summary";
import { Icon } from "../../lib/icons";
import MessageRow from "./MessageRow.vue";
import SystemNotice from "./SystemNotice.vue";
import ThinkingRow from "./ThinkingRow.vue";
import ToolRow from "./ToolRow.vue";
import TurnProcess from "./TurnProcess.vue";
import TurnTimerLine from "./TurnTimerLine.vue";
import UncertainCard from "./UncertainCard.vue";

const props = defineProps<{
  /** 待渲染条目（主对话 = chat.items；页签 = subagent_transcript 投影）。 */
  items: ChatItem[];
  /** 最后一轮执行中（缺省 false = 历史重放同形）。 */
  running?: boolean;
  /** 「已停止」标记来源（主对话 stopRequested）。 */
  stopRequested?: boolean;
  /** 可编辑用户气泡 id 集（§3.34；缺省 = 全只读）。 */
  editableIds?: Set<number>;
}>();

const emit = defineEmits<{
  edit: [pos: number | undefined, text: string];
  openTranscript: [childId: string, title: string];
  openDiff: [callId: string, path: string, diff: string];
  noticeAction: [item: SystemItem, action: string];
}>();

const { t } = useI18n();
const settings = useSettingsStore();
const approvals = useApprovalsStore();

/** 过程项（§3.7 v4.11 组内都是扁平行）：工具行 / 思考行 / 错误行。
 * 决策记录行 v2.98 全隐藏，不入组。 */
type ProcessItem = Extract<ChatItem, { kind: "tool" | "thinking" | "error" }>;
type TurnBlock = {
  kind: "turn";
  key: number;
  items: ProcessItem[];
  failures: number;
  toolCount: number;
  /** L1 极简摘要用：块内动作名（§3.18 中文动作名，去重保序）。 */
  names: string[];
  /** 组内有真过程（工具/思考）= 套组壳；纯错误行平铺不套壳。 */
  hasProcess: boolean;
  startTs?: number;
};
type Block =
  | TurnBlock
  | { kind: "single"; key: number; item: Exclude<ChatItem, ProcessItem> };

/** 连续过程项合成一组；其余条目各自为兄弟节点。 */
const blocks = computed<Block[]>(() => {
  const out: Block[] = [];
  let cur: ProcessItem[] = [];
  const flush = () => {
    if (cur.length === 0) return;
    const tools = cur.filter((i): i is ToolItem => i.kind === "tool");
    // 计时起点 = 组内最早的过程项时刻（工具与思考都算；错误行无时戳）
    const tsList = cur
      .map((i) => ("ts" in i ? i.ts : undefined))
      .filter((v): v is number => v !== undefined);
    // L1 极简摘要：块内动作名去重保序（§3.18 中文动作名，表外回落工具 id）
    const names: string[] = [];
    for (const i of tools) {
      const a = toolAction(i.tool);
      const base = a ? t(a.key, a.params ?? {}) : i.tool;
      // v2.105 裁定：派生子代理的两条告知框（已派生/收束）退役，简洁状态
      // 收进动作名后——L1 极简行读作「派生子代理 · 已完成」
      const label =
        i.tool === "subagent_run"
          ? `${base} · ${
              i.pending
                ? t("tool.running")
                : i.ok
                  ? t("turn.done")
                  : t("tool.failed")
            }`
          : base;
      if (!names.includes(label)) names.push(label);
    }
    out.push({
      kind: "turn",
      key: cur[0]!.id,
      items: cur,
      failures: tools.filter((i) => i.ok === false).length,
      toolCount: tools.length,
      names: names,
      hasProcess: cur.some((i) => i.kind === "tool" || i.kind === "thinking"),
      // 组内最早的过程项时刻（工具与思考都算——计时从思考开始）
      startTs: tsList.length > 0 ? Math.min(...tsList) : undefined,
    });
    cur = [];
  };
  for (const item of props.items) {
    // v2.98 裁定：审批决策记录行（「已允许 ×」每步反馈）全隐藏
    if (item.kind === "decision") continue;
    if (
      item.kind === "tool" ||
      item.kind === "thinking" ||
      item.kind === "error"
    ) {
      cur.push(item);
    } else {
      flush();
      out.push({ kind: "single", key: item.id, item });
    }
  }
  flush();
  return out;
});

/** 用户条目的事件位（§3.34 编辑重发定位键；模板窄化集中收窄）。 */
function userPos(item: ChatItem): number | undefined {
  return item.kind === "user" ? item.pos : undefined;
}

/** 「折叠工具执行过程」（v4.20 改名+语义反转）：开 = 过程组默认折叠为
 * 一行摘要（完整折叠三层）；关（缺省）= 过程组默认展开（行可见）——唯一
 * 被默认折叠的层是工具明细（层 3，恒收起，不再接开关）。实时/重放同口径。 */
const expandDefault = computed(() => !settings.view?.chat.collapseToolProcess);

/** 「已停止」只标最新过程组（stopRequested 轮次收束后保持到下次发送）。 */
const lastProcessKey = computed(() => {
  for (let i = blocks.value.length - 1; i >= 0; i--) {
    const b = blocks.value[i]!;
    if (b.kind === "turn" && b.hasProcess) return b.key;
  }
  return null;
});
function isStoppedBlock(key: number): boolean {
  return !!props.stopRequested && key === lastProcessKey.value;
}

// ---- 任务容器（v2.99「第 0 层」用户裁定）----
/** 每条用户消息开启一轮：容器内 = 轮计时行 + 该轮全部动作块与简短回复
 * （执行中恒展开）；容器外尾块 = 轮末尾连续的非过程块（最终报告 + 其后
 * 系统条目）——容器收缩后仍可见、最终报告有复制钮。调度注入轮没有实时
 * 用户消息 → 过程块自成一轮（计时行无读数，只剩发丝线，不虚构起点）。 */
type TurnView = {
  /** 容器身份 = 用户消息条目 id（调度轮 = 首个过程组键）。 */
  key: number;
  userItem?: Extract<ChatItem, { kind: "user" }>;
  /** 轮起点 = 用户消息实时时刻（计时行读数起点；undefined = 无从计时）。 */
  startTs?: number;
  /** 轮收束时刻（v2.101，turn_done 轮界事件投影——重放读数终点）。 */
  endTs?: number;
  running: boolean;
  inner: Block[];
  /** 容器外尾块（构造保证只剩 single——最终报告 + 其后系统条目）。 */
  tail: Extract<Block, { kind: "single" }>[];
};
const turnViews = computed<TurnView[]>(() => {
  const views: TurnView[] = [];
  let cur: {
    key: number;
    userItem?: Extract<ChatItem, { kind: "user" }>;
    blocks: Block[];
  } | null = null;
  const close = () => {
    if (!cur) return;
    // 尾块 = 末尾连续的非过程块；过程块一旦再现（调度轮续作）即归还容器
    let i = cur.blocks.length;
    while (i > 0 && cur.blocks[i - 1]!.kind === "single") i--;
    views.push({
      key: cur.key,
      userItem: cur.userItem,
      startTs: cur.userItem?.ts,
      endTs: cur.userItem?.endTs,
      running: false, // 最后一轮在收尾处统一回填
      inner: cur.blocks.slice(0, i),
      tail: cur.blocks.slice(i) as Extract<Block, { kind: "single" }>[],
    });
    cur = null;
  };
  for (const block of blocks.value) {
    if (block.kind === "single" && block.item.kind === "user") {
      close();
      cur = { key: block.key, userItem: block.item, blocks: [] };
    } else {
      if (!cur) cur = { key: block.key, blocks: [] };
      cur.blocks.push(block);
    }
  }
  close();
  if (views.length > 0) {
    views[views.length - 1]!.running = props.running ?? false;
  }
  return views;
});

/** 容器开合覆盖：手动点击优先。缺省 = 执行中恒开；收束后按「折叠工具
 * 执行过程」开关（v4.20 实机走查裁定补线：容器是第 0 层，不接开关则
 * 关档下轮次照样收缩成计时行，开关形同虚设）——关（缺省）= 容器保持
 * 展开整个轮次直接可见、唯一折叠的是工具明细；开 = 收缩为计时行（点击
 * 重开）。开关即时生效：已挂容器随保存重渲染。 */
const turnBoxOverride = ref(new Map<number, boolean>());
function turnBoxDefault(running: boolean): boolean {
  return running || !settings.view?.chat.collapseToolProcess;
}
function turnBoxOpen(key: number, running: boolean): boolean {
  return turnBoxOverride.value.get(key) ?? turnBoxDefault(running);
}
function toggleTurnBox(key: number, running: boolean) {
  const m = new Map(turnBoxOverride.value);
  m.set(key, !turnBoxOpen(key, running));
  turnBoxOverride.value = m;
}

/** 组相位（v2.99 层层折叠）：live = L3 执行中（组体全展开、无组头）；
 * replied = L2 回复已到（半折叠为一行摘要，仅当仍是当轮末位过程组）；
 * done = L1 已被下一动作接替 / 整轮收束（极简一行）。判定按轮内序列：
 * 末位块 = live（尚无回复）；末位过程组（回复已落在其后）= replied；
 * 其余全部 done。 */
type GroupPhase = "live" | "replied" | "done";
const phaseByKey = computed(() => {
  const m = new Map<number, GroupPhase>();
  for (const turn of turnViews.value) {
    const seq = [...turn.inner, ...turn.tail];
    const lastKey = seq[seq.length - 1]?.key;
    let lastProcKey: number | null = null;
    for (const b of seq) {
      if (b.kind === "turn" && b.hasProcess) lastProcKey = b.key;
    }
    for (const b of seq) {
      if (b.kind !== "turn" || !b.hasProcess) continue;
      if (!turn.running) m.set(b.key, "done");
      else if (b.key === lastKey) m.set(b.key, "live");
      else if (b.key === lastProcKey) m.set(b.key, "replied");
      else m.set(b.key, "done");
    }
  }
  return m;
});

/** 复制钮收敛（用户裁定）：只出现在用户消息与每轮最终汇报/总结之下——
 * 判据 = 该助手条目到下一个用户消息之间再无任何过程项（工具/思考/错误；
 * 决策记录行已隐藏不参与判据）；任务执行中间的简短回话后面跟着动作，
 * 不给复制钮。 */
const finalAssistantIds = computed(() => {
  const ids = new Set<number>();
  const items = props.items;
  for (let i = 0; i < items.length; i++) {
    const it = items[i]!;
    if (it.kind !== "assistant") continue;
    let followed = false;
    for (let j = i + 1; j < items.length; j++) {
      const n = items[j]!;
      if (n.kind === "user") break;
      if (n.kind === "tool" || n.kind === "thinking" || n.kind === "error") {
        followed = true;
        break;
      }
    }
    if (!followed) ids.add(it.id);
  }
  return ids;
});

/** 待决审批按 call_id 索引（工具行「等待审批」状态词的数据源；决策卡
 * 本体在 composer 坞，§3.17 v4.11）。 */
const pendingByCall = computed(() => {
  const m = new Map<string, boolean>();
  for (const p of approvals.pending) m.set(p.request_id, true);
  return m;
});
function isAwaiting(callId: string): boolean {
  return pendingByCall.value.has(callId);
}

/** 系统消息文本：__raw__ 条渲染原文，其余按 i18n key 翻译。 */
function noticeText(item: SystemItem): string {
  if (item.key === "__raw__") return String(item.params?.text ?? "");
  return t(item.key, item.params ?? {});
}
</script>

<template>
  <template v-for="turn of turnViews" :key="turn.key">
    <!-- 用户消息（容器外，§3.34 可编辑由父层给集；复制钮常态有）。
         parts（批 D）：附件缩略图随重放与实时同形恢复 -->
    <MessageRow
      v-if="turn.userItem"
      kind="user"
      :text="turn.userItem.text"
      :ts="turn.userItem.ts"
      :parts="turn.userItem.parts"
      :editable="editableIds?.has(turn.userItem.id) ?? false"
      @edit="(t2: string) => { if (turn.userItem) emit('edit', userPos(turn.userItem), t2); }"
    />

    <!-- 任务容器（第 0 层，v2.99 裁定）：计时行常驻头部（工作中/已
         工作 + 发丝线），执行中内容全开；收束收缩为计时行，点击重开
         逐层看全程。最终报告与其后系统条目在容器外尾段 -->
    <div class="turn-box">
      <TurnTimerLine
        :start-ts="turn.startTs"
        :end-ts="turn.endTs"
        :running="turn.running"
        :aria-expanded="turnBoxOpen(turn.key, turn.running)"
        @click="toggleTurnBox(turn.key, turn.running)"
      />
      <template v-if="turnBoxOpen(turn.key, turn.running)">
        <template v-for="block in turn.inner" :key="block.key">
          <!-- 动作块：L3 执行中全展开（无组头）/ L2 摘要行 / L1 极简
               行（折叠形态由相位机给，TurnProcess 内部落层） -->
          <template v-if="block.kind === 'turn'">
            <TurnProcess
              v-if="block.hasProcess"
              :stopped="isStoppedBlock(block.key)"
              :tool-count="block.toolCount"
              :failures="block.failures"
              :names="block.names"
              :start-ts="block.startTs"
              :phase="phaseByKey.get(block.key) ?? 'done'"
              :default-expanded="expandDefault"
            >
              <template v-for="item in block.items" :key="item.id">
                <ThinkingRow
                  v-if="item.kind === 'thinking'"
                  :text="item.text"
                  :ts="item.ts"
                  :done-ts="item.doneTs"
                />
                <ToolRow
                  v-else-if="item.kind === 'tool'"
                  :item="item"
                  :awaiting="isAwaiting(item.callId)"
                  @open-transcript="(id, title) => emit('openTranscript', id, title)"
                  @open-diff="(callId, path, diff) => emit('openDiff', callId, path, diff)"
                />
                <!-- 错误行入组平铺（裁定①：失败通知去卡片底） -->
                <div v-else class="flow-error-row">
                  <Icon name="warn" :size="12" class="shrink-0" />
                  <span>{{ item.text }}</span>
                </div>
              </template>
              <!-- v4.13：SubagentCard 大卡片退役——subagent_run 就是组内一列
                   工具行（§3.10 紧凑行），入口在行明细「查看完整对话」 -->
            </TurnProcess>
            <template v-else>
              <template v-for="item in block.items" :key="item.id">
                <div v-if="item.kind === 'error'" class="error-row">
                  <Icon name="warn" :size="14" class="shrink-0" />
                  <span>{{ item.text }}</span>
                </div>
              </template>
            </template>
          </template>

          <!-- 模型简短回复（容器内，无复制钮——复制钮只随最终报告）；
               模型正文不参与长消息折叠（v2.101 裁定，按 kind 判定） -->
          <MessageRow
            v-else-if="block.item.kind === 'assistant'"
            kind="assistant"
            :text="block.item.text"
            :ts="block.item.ts"
            :copyable="finalAssistantIds.has(block.item.id)"
          />
          <!-- 会话内系统消息（SystemNotice 模式）：组外兄弟节点 -->
          <div v-else-if="block.item.kind === 'system'" class="sys-row flex justify-center">
            <SystemNotice
              :text="noticeText(block.item as SystemItem)"
              :actions="
                (block.item.actions ?? []).map((a) => ({
                  id: a,
                  label: t(`notice.action.${a}`),
                }))
              "
              @action="(a: string) => emit('noticeAction', block.item as SystemItem, a)"
            />
          </div>
          <!-- G2 崩溃恢复 uncertain 决策卡：组外兄弟（折进组会被默认折叠
               藏住，决策面必须可见）；resolved 后收为记录行 -->
          <div v-else-if="block.item.kind === 'uncertain'" class="unc-row">
            <UncertainCard :item="block.item" />
          </div>
        </template>
      </template>
    </div>

    <!-- 容器外尾块：最终报告（复制钮只在此）与其后系统条目 -->
    <template v-for="block in turn.tail" :key="block.key">
      <MessageRow
        v-if="block.item.kind === 'assistant'"
        kind="assistant"
        :text="block.item.text"
        :ts="block.item.ts"
        :copyable="finalAssistantIds.has(block.item.id)"
      />
      <div v-else-if="block.item.kind === 'system'" class="sys-row flex justify-center">
        <SystemNotice
          :text="noticeText(block.item as SystemItem)"
          :actions="
            (block.item.actions ?? []).map((a) => ({
              id: a,
              label: t(`notice.action.${a}`),
            }))
          "
          @action="(a: string) => emit('noticeAction', block.item as SystemItem, a)"
        />
      </div>
      <!-- G2：崩溃会话没有最终报告，uncertain 卡常落尾段（同组外语义） -->
      <div v-else-if="block.item.kind === 'uncertain'" class="unc-row">
        <UncertainCard :item="block.item" />
      </div>
    </template>
  </template>
</template>

<style scoped>
/* 任务容器（v2.99「第 0 层」）：计时行常驻头部 + 动作块/简短回复；收束
 * 收缩为计时行，最终报告在容器外——容器↔报告间距归兄弟规则（v4.15 用户
 * 裁定 12px：全 8px 分不出最终报告起头；旧 16px 容器底 padding + 报告自
 * 带 12px = 28px 退役） */
.turn-box {
  padding-bottom: 0;
}
/* 间距（v4.14 用户裁定：三层间距统一 8px——动作块之间、动作块↔过渡文本、
 * 过渡文本→下一动作块全部同值；旧 8/8/12 三值退役。跨组件兄弟选择器
 * （组件根都带本层 data-v）；.turn 垂直 padding 已归零，间距全由这
 * 里给。计时行属容器 chrome（第 0 层），不在三层之列，保持 2px）。
 * 简短回复 .msg--brief 自身 padding 归零（2026-10-07：隐藏时间行占位 +
 * 自带 12px padding 与此处 8px 双重计算，实际间距曾达 24-30px）——相邻
 * 对在这里补全：简短↔简短、简短/动作块↔系统条目双向。 */
.turn + .msg {
  padding-top: var(--lw-gap-8);
}
.msg + .turn {
  padding-top: var(--lw-gap-8);
}
.turn + .turn {
  padding-top: var(--lw-gap-8);
}
.msg + .msg,
.msg + .sys-row,
.sys-row + .msg,
.sys-row + .sys-row,
.turn + .sys-row,
.sys-row + .turn,
.msg + .unc-row,
.unc-row + .msg,
.turn + .unc-row,
.unc-row + .turn,
.sys-row + .unc-row,
.unc-row + .sys-row,
.unc-row + .unc-row {
  padding-top: var(--lw-gap-8);
}
.turn-timer + .turn,
.turn-timer + .msg {
  padding-top: var(--lw-gap-2);
}
.turn-timer + .sys-row {
  padding-top: var(--lw-gap-2);
}
/* 容器（第 0 层）↔ 容器外尾块（最终报告 / 其后系统条目）= 12px（v4.15
 * 用户裁定：节奏整体 8px，唯此处加大一档让最终报告的起头可分辨）；下一
 * 轮用户消息紧跟容器时同规 */
.turn-box + .msg,
.turn-box + .sys-row {
  padding-top: var(--lw-gap-12);
}
.error-row {
  display: flex;
  align-items: flex-start;
  gap: 7px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值7,§二⑬) */
  padding: var(--lw-gap-2) 0;
  font-size: var(--lw-fs-12);
  line-height: 1.5;
  color: var(--lw-danger);
}
/* v4.13 与组内平铺错误行同形（去卡片底——红线「不要卡片」）；与栅格行同一左缘 */
.error-row span {
  min-width: 0;
  overflow-wrap: anywhere;
}
.error-row > svg {
  color: var(--lw-danger);
}
/* §3.7 组内平铺错误行：无卡片底，扁平左对齐（与工具行/思考行同一左缘）；
 * 决策记录行已全隐藏（v2.98 裁定），其平铺样式随之退役 */
.flow-error-row {
  display: flex;
  align-items: flex-start;
  gap: 7px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值7,§二⑬) */
  padding: var(--lw-gap-2) 0;
  font-size: var(--lw-fs-12);
  line-height: 1.5;
  color: var(--lw-danger);
}
.flow-error-row span {
  min-width: 0;
  overflow-wrap: anywhere;
}
.flow-error-row > svg {
  color: var(--lw-danger);
}
</style>
