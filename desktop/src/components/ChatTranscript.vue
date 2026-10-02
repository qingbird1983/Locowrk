<script setup lang="ts">
/**
 * 会话转录（UI shell doc §8.4 ChatTranscript，由 ChatMessages.vue 拆出）：
 * 滚动区（overscroll contain / scrollbar-gutter stable / overflow-anchor
 * none）+ 列（宽 = --lw-content-max 覆盖值，§2.7 三处同源）+ 底部渐隐
 * mask（内容浮到输入卡下方时淡出，不盖不透明底带）。条目按 §3.7 分组：
 * 连续的过程项（工具行 / 思考行）合成轮次过程组；系统通知、助手正文、
 * 错误行是组的兄弟节点——组折起来不藏结论。
 */
import { computed, nextTick, ref, watch } from "vue";
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
import { useWorkPanelStore } from "../stores/work-panel";
import { clip, formatDuration } from "../lib/format";
import { Icon } from "../lib/icons";
import type { ApprovalRequest } from "../types";
import ConversationWidthHandles from "./chat/ConversationWidthHandles.vue";
import MessageRow from "./chat/MessageRow.vue";
import PermissionCard from "./chat/PermissionCard.vue";
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

/** §3.10：子 Agent 卡是入口——点「看转录」开工作面板 §3.16 页签。 */
function onOpenTranscript(childId: string, title: string) {
  workPanel.openSubTab(childId, clip(title, 24));
}

/** §3.28：edit_diff 卡「看 diff」开工作面板审阅页签。 */
function onOpenDiff(callId: string, path: string, diff: string) {
  workPanel.openReviewTab(callId, path, diff);
}
const bottom = ref<HTMLElement | null>(null);

const props = defineProps<{
  /** composer 坞总高（px）：列宽热区下沿 + 底部渐隐 mask 的锚。 */
  composerBottom: number;
}>();

/** 过程项（§3.7 组内只有过程）：工具行 / 思考行。 */
type ProcessItem = Extract<ChatItem, { kind: "tool" | "thinking" }>;
type TurnBlock = {
  kind: "turn";
  key: number;
  items: ProcessItem[];
  running: boolean;
  failures: number;
  toolCount: number;
  startTs?: number;
  /** §3.10 子 Agent 卡数据（组内 subagent_run 调用，卡是 §3.16 入口）。 */
  subagents: SubagentChild[];
};
type Block =
  | TurnBlock
  | { kind: "single"; key: number; item: Exclude<ChatItem, ProcessItem> };

/** 连续过程项（工具/思考）合成一组；其余条目各自为兄弟节点。 */
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
      running: tools.some((i) => i.pending),
      failures: tools.filter((i) => i.ok === false).length,
      toolCount: tools.length,
      startTs: tsList.length > 0 ? Math.min(...tsList) : undefined,
      subagents,
    });
    cur = [];
  };
  for (const item of chat.items) {
    if (item.kind === "tool" || item.kind === "thinking") cur.push(item);
    else {
      flush();
      out.push({ kind: "single", key: item.id, item });
    }
  }
  flush();
  return out;
});

/** 「默认展开工具执行过程」只对实时新发的轮次生效（ts 存在 = 实时）；
 * 历史重放一律折叠（§3.8）。 */
const expandLive = computed(
  () => !!settings.view?.chat.expandToolProcess && !chat.replaying,
);

/** 待决审批按 call_id 索引（request_id == call_id，§3.17 内联卡紧跟
 * 触发它的工具行）。 */
const pendingByCall = computed(() => {
  const m = new Map<string, ApprovalRequest>();
  for (const p of approvals.pending) m.set(p.request_id, p);
  return m;
});
function pendingFor(callId: string): ApprovalRequest | undefined {
  return pendingByCall.value.get(callId);
}
/** 找不到宿主工具行的待决卡（如调度注入的其他会话轮）兜底挂转录尾——
 * 不允许决策卡无声消失。 */
const orphanPending = computed(() =>
  approvals.pending.filter(
    (p) =>
      !chat.items.some((i) => i.kind === "tool" && i.callId === p.request_id),
  ),
);

watch(
  () => chat.items.length,
  async () => {
    await nextTick();
    bottom.value?.scrollIntoView({ behavior: "smooth" });
  },
);

/** 系统消息文本：__raw__ 条渲染原文，其余按 i18n key 翻译。 */
function noticeText(item: SystemItem): string {
  if (item.key === "__raw__") return String(item.params?.text ?? "");
  return t(item.key, item.params ?? {});
}

/** 系统消息动作（⑨ 接线）：details = 展开详情；undo = 撤销最近一次压缩。 */
async function onNoticeAction(item: SystemItem, action: string) {
  if (action === "details") {
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
    <div class="scroller scroll-y">
      <div class="column conv-w">
        <!-- 首页空态（§3.14）：吉祥物 + 一行问句，垂直居中于转录区，
             composer 仍在底部原位（启动落首页，㉕·对话区①） -->
        <div v-if="chat.items.length === 0" class="home">
          <svg
            class="home__mascot"
            viewBox="0 0 72 72"
            fill="none"
            stroke="currentColor"
            stroke-width="1.5"
            stroke-linecap="round"
            stroke-linejoin="round"
            aria-hidden="true"
          >
            <path
              d="M22 47.5a9.5 9.5 0 0 1-1.4-18.9 13 13 0 0 1 25.2-4.6 10 10 0 0 1 1.7 23.5Z"
            />
            <circle cx="30.6" cy="32.4" r="1.9" fill="currentColor" stroke="none" />
            <circle cx="41.4" cy="32.4" r="1.9" fill="currentColor" stroke="none" />
          </svg>
          <h1 class="home__title">{{ t("chat.homeTitle") }}</h1>
        </div>

        <template v-for="block in blocks" :key="block.key">
          <!-- 轮次过程组：连续过程项（工具/思考）；组内只有过程 -->
          <template v-if="block.kind === 'turn'">
            <TurnProcess
              :running="block.running"
              :tool-count="block.toolCount"
              :failures="block.failures"
              :start-ts="block.startTs"
              :default-expanded="expandLive && block.startTs !== undefined"
            >
              <template v-for="item in block.items" :key="item.id">
                <ThinkingRow v-if="item.kind === 'thinking'" :text="item.text" />
                <template v-else>
                  <ToolRow
                    :item="item"
                    :awaiting="!!pendingFor(item.callId)"
                    :default-expanded="expandLive && item.ts !== undefined"
                    @open-transcript="onOpenTranscript"
                    @open-diff="onOpenDiff"
                  />
                  <!-- §3.17 决策卡紧跟触发它的工具行（内联，不是弹窗） -->
                  <PermissionCard
                    v-if="pendingFor(item.callId)"
                    :req="pendingFor(item.callId)!"
                    :queue="approvals.queueCount"
                  />
                </template>
              </template>
              <!-- §3.10 子 Agent 卡：组内 subagent_run 的可视形态 + §3.16 入口 -->
              <SubagentCard
                v-if="block.subagents.length > 0"
                :children="block.subagents"
                @open="onOpenTranscript"
              />
            </TurnProcess>
          </template>

          <template v-else>
            <MessageRow
              v-if="block.item.kind === 'user'"
              kind="user"
              :text="block.item.text"
              :ts="block.item.ts"
            />
            <MessageRow
              v-else-if="block.item.kind === 'assistant'"
              kind="assistant"
              :text="block.item.text"
              :ts="block.item.ts"
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

            <!-- 错误行（§3.7：错误行也是组的兄弟节点） -->
            <div v-else-if="block.item.kind === 'error'" class="error-row">
              <Icon name="warn" :size="14" class="shrink-0" />
              <span>{{ block.item.text }}</span>
            </div>
          </template>
        </template>

        <!-- 无宿主行的待决卡兜底（§3.17：不允许决策卡无声消失） -->
        <PermissionCard
          v-for="req in orphanPending"
          :key="req.request_id"
          :req="req"
          :queue="approvals.queueCount"
        />
        <div ref="bottom" />
      </div>
    </div>

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
  scrollbar-gutter: stable;
  overflow-anchor: none;
  padding: 8px 24px 0;
  /* 底部渐隐：内容浮到输入卡下方时淡出（遮罩盖住 composer 预留区） */
  -webkit-mask-image: linear-gradient(
    to bottom,
    #000 0,
    #000 calc(100% - 168px),
    rgb(0 0 0 / 0.25) calc(100% - 128px),
    transparent calc(100% - 100px)
  );
  mask-image: linear-gradient(
    to bottom,
    #000 0,
    #000 calc(100% - 168px),
    rgb(0 0 0 / 0.25) calc(100% - 128px),
    transparent calc(100% - 100px)
  );
}
.column {
  width: min(100%, var(--lw-content-max));
  margin-inline: auto;
  padding-bottom: 180px;
}
/* 首页空态（§3.14 / 原型 S1）：单线云朵 + 26px 一行问句（400 / -0.02em），
 * 垂直居中于转录区；composer 留在底部原位 */
.home {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 20px;
  /* 顶栏 46 + composer 坞 ≈190 + 底部渐隐 mask 预留 */
  min-height: calc(100vh - 46px - 236px);
}
.home__mascot {
  width: 72px;
  height: 72px;
  color: var(--lw-ink-1);
}
.home__title {
  font-size: 26px;
  font-weight: 400;
  letter-spacing: -0.02em;
  line-height: 1.2;
  color: var(--lw-ink-1);
  text-align: center;
}
.error-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  margin: 8px 0;
  border-radius: var(--lw-r-sm);
  border: 1px solid color-mix(in oklab, var(--lw-danger) 30%, transparent);
  background: color-mix(in oklab, var(--lw-danger) 8%, transparent);
  padding: 8px 12px;
  font-size: 12px;
  line-height: 1.45;
  color: var(--lw-danger);
}
</style>
