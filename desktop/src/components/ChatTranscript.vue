<script setup lang="ts">
/**
 * 会话转录（UI shell doc §8.4 ChatTranscript）：滚动区（overscroll contain /
 * scrollbar-gutter stable / overflow-anchor none）+ 列（宽 = --lw-content-max
 * 覆盖值，§2.7 三处同源）+ 底部渐隐 mask。条目按 §3.7 v4.11 分组：连续的
 * 过程项（工具行 / 思考行 / 决策记录行 / 错误行）合成轮次过程组，组内扁平
 * 左对齐（裁定⑪）；纯决策/错误无过程项时不套组壳直接平铺。助手正文与系统
 * 通知是组的兄弟节点。
 *
 * v4.11：审批卡迁坞（Composer 槽位，§3.17），转录里只剩决策记录行；
 * ⑤⑨ 跟随滚动（默认贴底，用户上滚即让位）+ 回到底部钮（坞上方）。
 */
import { computed, nextTick, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { contextUndoCompact } from "../lib/api";
import { useApprovalsStore } from "../stores/approvals";
import {
  useChatStore,
  type ChatItem,
  type SystemItem,
  type ToolItem,
} from "../stores/chat";
import { useSettingsStore } from "../stores/settings";
import { useShellStore } from "../stores/shell";
import { useWorkPanelStore } from "../stores/work-panel";
import { clip, formatDuration } from "../lib/format";
import { toolAction } from "../lib/tool-summary";
import { Icon } from "../lib/icons";
import HoverTip from "./settings/HoverTip.vue";
import ConversationWidthHandles from "./chat/ConversationWidthHandles.vue";
import MessageRow from "./chat/MessageRow.vue";
import SubagentCard, { type SubagentChild } from "./chat/SubagentCard.vue";
import SystemNotice from "./chat/SystemNotice.vue";
import ThinkingRow from "./chat/ThinkingRow.vue";
import ToolRow from "./chat/ToolRow.vue";
import TurnProcess from "./chat/TurnProcess.vue";

const { t } = useI18n();
const chat = useChatStore();
const settings = useSettingsStore();
const approvals = useApprovalsStore();
const workPanel = useWorkPanelStore();

/** §3.18 动作名（决策记录行用）；表外工具回落工具 id。 */
function actionLabel(tool: string): string {
  const a = toolAction(tool);
  return a ? t(a.key, a.params ?? {}) : tool;
}

/** §3.10：子 Agent 卡是入口——点「看转录」开工作面板 §3.16 页签。 */
function onOpenTranscript(childId: string, title: string) {
  workPanel.openSubTab(childId, clip(title, 24));
}

/** §3.28：edit_diff 卡「看 diff」开工作面板审阅页签。 */
function onOpenDiff(callId: string, path: string, diff: string) {
  workPanel.openReviewTab(callId, path, diff);
}

const props = defineProps<{
  /** composer 坞总高（px）：列宽热区下沿 + 底部渐隐 mask 的锚。 */
  composerBottom: number;
}>();

/** 过程项（§3.7 v4.11 组内都是扁平行）：工具行 / 思考行 / 决策记录 /
 * 错误行。 */
type ProcessItem = Extract<
  ChatItem,
  { kind: "tool" | "thinking" | "decision" | "error" }
>;
type TurnBlock = {
  kind: "turn";
  key: number;
  items: ProcessItem[];
  running: boolean;
  failures: number;
  toolCount: number;
  /** 组内有真过程（工具/思考）= 套组壳；纯决策/错误行平铺不套壳。 */
  hasProcess: boolean;
  startTs?: number;
  /** §3.10 子 Agent 卡数据（组内 subagent_run 调用，卡是 §3.16 入口）。 */
  subagents: SubagentChild[];
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
    const tsList = tools.map((i) => i.ts).filter((v): v is number => v !== undefined);
    const subagents: SubagentChild[] = [];
    for (const i of tools) {
      if (i.tool !== "subagent_run") continue;
      const childId = (i.output as { child_session_id?: string } | null)
        ?.child_session_id;
      // 未收束的调用还没有 child_id（悬空调用先行落事件）——卡里不列空行
      if (!childId) continue;
      subagents.push({
        childId,
        prompt: clip(String((i.args as { prompt?: unknown } | null)?.prompt ?? ""), 24),
        status: i.pending ? "running" : i.ok ? "done" : "failed",
        durationText:
          i.ts !== undefined && i.doneTs !== undefined
            ? formatDuration(i.doneTs - i.ts)
            : undefined,
      });
    }
    out.push({
      kind: "turn",
      key: cur[0]!.id,
      items: cur,
      // 组在跑 = 有 pending 工具，或末尾思考行还在流（§3.9：实时 ts 在、
      // 回复/工具到达即收口 doneTs——不并入的话思考阶段相位= replied，
      // 思考行被折叠藏住，v2.97 走查实抓）
      running:
        tools.some((i) => i.pending) ||
        cur.some(
          (i) =>
            i.kind === "thinking" && i.ts !== undefined && i.doneTs === undefined,
        ),
      failures: tools.filter((i) => i.ok === false).length,
      toolCount: tools.length,
      hasProcess: cur.some((i) => i.kind === "tool" || i.kind === "thinking"),
      startTs: tsList.length > 0 ? Math.min(...tsList) : undefined,
      subagents,
    });
    cur = [];
  };
  for (const item of chat.items) {
    if (
      item.kind === "tool" ||
      item.kind === "thinking" ||
      item.kind === "decision" ||
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

/** §3.34 可编辑的用户气泡：重放消息（带事件位）或当前最后一条用户消息
 * （实时发送无事件位的解析口）；运行中全部不可编辑。 */
const editableUserIds = computed(() => {
  const ids = new Set<number>();
  if (chat.running) return ids;
  let lastUserIdx = -1;
  chat.items.forEach((it, i) => {
    if (it.kind === "user") lastUserIdx = i;
  });
  chat.items.forEach((it, i) => {
    if (it.kind !== "user") return;
    if (it.pos !== undefined || i === lastUserIdx) ids.add(it.id);
  });
  return ids;
});

/** 用户条目的事件位（模板窄化不进嵌套函数，这里集中收窄）。 */
function userPos(item: ChatItem): number | undefined {
  return item.kind === "user" ? item.pos : undefined;
}

/** §3.34 编辑重发：截断失败（运行中/后端拒绝）以系统条如实呈现。 */
async function onEditResend(pos: number | undefined, text: string) {
  try {
    await chat.editResend(pos ?? null, text);
  } catch (err) {
    chat.pushNotice("notice.editResendFailed", { reason: String(err) });
  }
}

/** 「默认展开工具执行过程」v4.11 覆盖重放（裁定②：所有过程组含历史
 * 一律默认折叠，开关是唯一的展开缺省）。 */
const expandDefault = computed(() => !!settings.view?.chat.expandToolProcess);

/** 「已停止」只标最新过程组（stopRequested 轮次收束后保持到下次发送）。 */
const lastProcessKey = computed(() => {
  for (let i = blocks.value.length - 1; i >= 0; i--) {
    const b = blocks.value[i]!;
    if (b.kind === "turn" && b.hasProcess) return b.key;
  }
  return null;
});
function isStoppedBlock(key: number): boolean {
  return chat.stopRequested && key === lastProcessKey.value;
}

// ---- 四层折叠状态机（v2.96 用户裁定：L3 最新动作流式 / L2 动作折行 /
// L1 组头 / L0 耗时+发丝线）----
/** 组相位：live = 本组有动作在跑；replied = 动作完、模型回话中（含同轮
 * 被新动作块接替的靠前组）；done = 整轮收束（running 结束全员 done）。 */
type GroupPhase = "live" | "replied" | "done";
const phaseByKey = computed(() => {
  const m = new Map<number, GroupPhase>();
  if (!chat.running) {
    for (const b of blocks.value) {
      if (b.kind === "turn" && b.hasProcess) m.set(b.key, "done");
    }
    return m;
  }
  const proc = blocks.value.filter(
    (b): b is TurnBlock => b.kind === "turn" && b.hasProcess,
  );
  proc.forEach((b) => {
    if (b.running) m.set(b.key, "live");
    // 动作已完：最后一组 = 模型正在回话；靠前组 = 已被后续动作块接替——
    // 两者同收 L1（replied），整轮 running 结束再一起进 done
    else m.set(b.key, "replied");
  });
  return m;
});

/** L3 聚焦 = live 组里最后一个 pending 工具（流式中的动作）。 */
function focusToolId(b: TurnBlock): number | null {
  if (!b.running) return null;
  for (let i = b.items.length - 1; i >= 0; i--) {
    const it = b.items[i]!;
    if (it.kind === "tool" && it.pending) return it.id;
  }
  return null;
}

/** 复制钮收敛（用户裁定）：只出现在用户消息与每轮最终汇报/总结之下——
 * 判据 = 该助手条目到下一个用户消息之间再无任何过程项（工具/思考/决策/
 * 错误）；任务执行中间的简短回话后面跟着动作，不给复制钮。 */
const finalAssistantIds = computed(() => {
  const ids = new Set<number>();
  const items = chat.items;
  for (let i = 0; i < items.length; i++) {
    const it = items[i]!;
    if (it.kind !== "assistant") continue;
    let followed = false;
    for (let j = i + 1; j < items.length; j++) {
      const n = items[j]!;
      if (n.kind === "user") break;
      if (
        n.kind === "tool" ||
        n.kind === "thinking" ||
        n.kind === "decision" ||
        n.kind === "error"
      ) {
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

// ---- §3.39③⑤ 跟随滚动 + 回到底部（v4.11 裁定⑨）----
// 默认贴底：任何条目变化（含流式 delta）都滚到底；用户上滚（距底
// > 48px）即让位，回到底部钮在 composer 坞上方伺服。
const scroller = ref<HTMLElement | null>(null);
const following = ref(true);
const BOTTOM_EPSILON = 48;
function onScroll() {
  const el = scroller.value;
  if (!el) return;
  following.value =
    el.scrollHeight - el.scrollTop - el.clientHeight <= BOTTOM_EPSILON;
}
async function scrollToBottom(smooth = false) {
  await nextTick();
  const el = scroller.value;
  if (!el) return;
  el.scrollTo({ top: el.scrollHeight, behavior: smooth ? "smooth" : "auto" });
}
function resumeFollow() {
  following.value = true;
  void scrollToBottom(true);
}
// watch(chat.items)（隐式深）：流式 delta 改写末条文本也触发——跟随滚动
// 不只看条目数。运行中即时贴底（流式平滑滚动会脱拍），静止变化平滑。
watch(chat.items, () => {
  if (following.value) void scrollToBottom(!chat.running);
});
watch(
  () => chat.sessionId,
  () => {
    following.value = true;
    void scrollToBottom();
  },
);
onMounted(() => void scrollToBottom());
// 切回对话目的地（§2.6 display:none 挂起）时视口尺寸曾不可用，回来自动贴底
watch(
  () => useShellStore().destination,
  (d) => {
    if (d === "chat") {
      following.value = true;
      void scrollToBottom();
    }
  },
);

/** 系统消息文本：__raw__ 条渲染原文，其余按 i18n key 翻译。 */
function noticeText(item: SystemItem): string {
  if (item.key === "__raw__") return String(item.params?.text ?? "");
  return t(item.key, item.params ?? {});
}

/** 系统消息动作（⑨ 接线）：details = 展开详情；undo = 撤销最近一次压缩；
 * view（M3 §3.37③）= 跳记忆页看整理产物（记忆树区块）。 */
async function onNoticeAction(item: SystemItem, action: string) {
  if (action === "view") {
    useShellStore().setDestination("memory");
  } else if (action === "details") {
    const detail = chat.noticeDetails[item.id];
    if (detail) chat.pushRawSystem(detail);
  } else if (action === "undo") {
    try {
      await contextUndoCompact(chat.sessionId);
      chat.pushNotice("notice.compactUndone");
    } catch (err) {
      chat.pushNotice("notice.compactUndoFailed", { reason: String(err) });
    }
  }
}
</script>

<template>
  <div class="relative min-h-0 flex-1">
    <div ref="scroller" class="scroller scroll-y" @scroll.passive="onScroll">
      <div class="column conv-w">
        <template v-for="block in blocks" :key="block.key">
          <!-- 轮次过程组：连续过程项扁平直排（§3.7 v4.11 裁定⑪ 同一左缘）；
               纯决策/错误（无工具/思考）不套组壳直接平铺 -->
          <template v-if="block.kind === 'turn'">
            <TurnProcess
              v-if="block.hasProcess"
              :running="block.running"
              :stopped="isStoppedBlock(block.key)"
              :tool-count="block.toolCount"
              :failures="block.failures"
              :start-ts="block.startTs"
              :phase="phaseByKey.get(block.key) ?? 'done'"
              :default-expanded="expandDefault"
            >
              <template v-for="item in block.items" :key="item.id">
                <ThinkingRow v-if="item.kind === 'thinking'" :text="item.text" />
                <ToolRow
                  v-else-if="item.kind === 'tool'"
                  :item="item"
                  :awaiting="isAwaiting(item.callId)"
                  :default-expanded="expandDefault"
                  :live-focus="focusToolId(block) === item.id"
                  @open-transcript="onOpenTranscript"
                  @open-diff="onOpenDiff"
                />
                <!-- §3.7 v4.11 决策记录行：无卡片底，组内平铺（裁定①） -->
                <div
                  v-else-if="item.kind === 'decision'"
                  class="decision-row"
                  :class="{ 'is-denied': !item.allowed }"
                >
                  <Icon
                    :name="item.allowed ? 'check' : 'warn'"
                    :size="12"
                    class="shrink-0"
                  />
                  <span>{{
                    t(item.allowed ? "notice.approvalAllowed" : "notice.approvalDenied", {
                      action: actionLabel(item.tool),
                    })
                  }}</span>
                </div>
                <!-- 错误行同样入组平铺（裁定①：失败通知去卡片底） -->
                <div v-else class="flow-error-row">
                  <Icon name="warn" :size="12" class="shrink-0" />
                  <span>{{ item.text }}</span>
                </div>
              </template>
              <!-- §3.10 子 Agent 卡：组内 subagent_run 的可视形态 + §3.16 入口 -->
              <SubagentCard
                v-if="block.subagents.length > 0"
                :children="block.subagents"
                @open="onOpenTranscript"
              />
            </TurnProcess>
            <template v-else>
              <template v-for="item in block.items" :key="item.id">
                <div
                  v-if="item.kind === 'decision'"
                  class="decision-row"
                  :class="{ 'is-denied': !item.allowed }"
                >
                  <Icon
                    :name="item.allowed ? 'check' : 'warn'"
                    :size="12"
                    class="shrink-0"
                  />
                  <span>{{
                    t(item.allowed ? "notice.approvalAllowed" : "notice.approvalDenied", {
                      action: actionLabel(item.tool),
                    })
                  }}</span>
                </div>
                <div v-else-if="item.kind === 'error'" class="error-row">
                  <Icon name="warn" :size="14" class="shrink-0" />
                  <span>{{ item.text }}</span>
                </div>
              </template>
            </template>
          </template>

          <template v-else>
            <MessageRow
              v-if="block.item.kind === 'user'"
              kind="user"
              :text="block.item.text"
              :ts="block.item.ts"
              :editable="editableUserIds.has(block.item.id)"
              @edit="(t: string) => onEditResend(userPos(block.item), t)"
            />
            <MessageRow
              v-else-if="block.item.kind === 'assistant'"
              kind="assistant"
              :text="block.item.text"
              :ts="block.item.ts"
              :copyable="finalAssistantIds.has(block.item.id)"
            />

            <!-- 会话内系统消息（SystemNotice 模式）：组外兄弟节点 -->
            <div
              v-else-if="block.item.kind === 'system'"
              class="flex justify-center"
            >
              <SystemNotice
                :text="noticeText(block.item as SystemItem)"
                :actions="
                  (block.item.actions ?? []).map((a) => ({
                    id: a,
                    label: t(`notice.action.${a}`),
                  }))
                "
                @action="(a: string) => onNoticeAction(block.item as SystemItem, a)"
              />
            </div>
          </template>
        </template>
      </div>
    </div>

    <!-- §3.39③⑤ 回到底部（v4.11）：用户上滚让位后伺服，坞上方居中 -->
    <HoverTip
      class="!static"
      :text="t('chat.backToLatest')"
      :label="t('chat.backToLatest')"
    >
      <Transition name="lw-pop-up">
        <button
          v-if="!following"
          class="back-bottom"
          type="button"
          :aria-label="t('chat.backToLatest')"
          :style="{ bottom: `${props.composerBottom + 12}px` }"
          @click="resumeFollow"
        >
          <Icon name="chevronDown" :size="14" />
        </button>
      </Transition>
    </HoverTip>

    <!-- 列宽双柄：容器下沿停在 composer 顶部（§2.7） -->
    <ConversationWidthHandles :bottom="props.composerBottom" />
  </div>
</template>

<style scoped>
.scroller {
  flex: 1 1 auto;
  height: 100%;
  min-height: 0;
  overflow-y: auto;
  overscroll-behavior: contain;
  /* both-edges（v2.60 修复批）：滚动条槽两侧对称预留——转录列与 composer
   * 列同以主区中线居中，正文不再整体左偏（旧 stable 只在右缘留槽，列中心
   * 左移半个槽宽） */
  scrollbar-gutter: stable both-edges;
  overflow-anchor: none;
  padding: var(--lw-gap-8) var(--lw-gap-24) 0;
  /* 底部渐隐：内容浮到输入卡下方时淡出（遮罩盖住 composer 预留区）。
   * 渐隐色 = --lw-mask-fade 令牌（§六标准：.vue 禁字面色） */
  -webkit-mask-image: linear-gradient(
    to bottom,
    var(--lw-mask-fade) 0,
    var(--lw-mask-fade) calc(100% - 168px),
    color-mix(in oklab, var(--lw-mask-fade) 25%, transparent) calc(100% - 128px),
    transparent calc(100% - 100px)
  );
  mask-image: linear-gradient(
    to bottom,
    var(--lw-mask-fade) 0,
    var(--lw-mask-fade) calc(100% - 168px),
    color-mix(in oklab, var(--lw-mask-fade) 25%, transparent) calc(100% - 128px),
    transparent calc(100% - 100px)
  );
}
.column {
  width: min(100%, var(--lw-content-max));
  margin-inline: auto;
  padding-bottom: 180px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值180,§二⑬) */
}
/* 四层折叠（v2.96）：动作块与随后的简短回话行距收紧——原链 10+12=22px，
 * 收为 6+4=10px（跨组件兄弟选择器：两组件根都带本层 data-v） */
.turn + .msg {
  padding-top: var(--lw-gap-4);
}
/* 首页空态（§3.14 v3.9）迁去 chat/WelcomeHome（欢迎页独立形态，composer
 * 随行）；本组件只在会话态挂载 */
.error-row {
  display: flex;
  align-items: flex-start;
  gap: var(--lw-gap-8);
  margin: var(--lw-gap-8) 0;
  border-radius: var(--lw-r-sm);
  border: 1px solid color-mix(in oklab, var(--lw-danger) 30%, transparent);
  background: color-mix(in oklab, var(--lw-danger) 8%, transparent);
  padding: var(--lw-gap-8) var(--lw-gap-12);
  font-size: var(--lw-fs-12);
  line-height: 1.45;
  color: var(--lw-danger);
}
/* §3.7 v4.11（裁定①）：组内平铺行——决策记录 / 错误，无卡片底，扁平
 * 左对齐（与工具行/思考行同一左缘）；ok=check 墨绿、denied/error=warn 红 */
.decision-row,
.flow-error-row {
  display: flex;
  align-items: flex-start;
  gap: 7px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值7,§二⑬) */
  padding: var(--lw-gap-2) 0;
  font-size: var(--lw-fs-12);
  line-height: 1.5;
  color: var(--lw-ink-2);
}
.decision-row span,
.flow-error-row span {
  min-width: 0;
  overflow-wrap: anywhere;
}
.decision-row > svg {
  color: var(--lw-ok);
  margin-top: var(--lw-gap-2);
}
.decision-row.is-denied > svg,
.flow-error-row > svg {
  color: var(--lw-danger);
}
.decision-row.is-denied,
.flow-error-row {
  color: var(--lw-danger);
}
/* §3.39③ 回到底部钮：32px 圆钮，composer 坞上方居中（bottom 动态 =
 * 坞高 + 12px）；z 高于渐隐 mask */
.back-bottom {
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
  width: 32px;
  height: 32px;
  border-radius: var(--lw-r-full);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-raised);
  color: var(--lw-ink-2);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  z-index: 13;
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.back-bottom:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
</style>
