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
import { useChatStore, type ChatItem, type SystemItem, type ToolItem } from "../stores/chat";
import { useSettingsStore } from "../stores/settings";
import { Icon } from "../lib/icons";
import ConversationWidthHandles from "./chat/ConversationWidthHandles.vue";
import MessageRow from "./chat/MessageRow.vue";
import SystemNotice from "./chat/SystemNotice.vue";
import ThinkingRow from "./chat/ThinkingRow.vue";
import ToolRow from "./chat/ToolRow.vue";
import TurnProcess from "./chat/TurnProcess.vue";

const { t } = useI18n();
const chat = useChatStore();
const settings = useSettingsStore();
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
    out.push({
      kind: "turn",
      key: cur[0]!.id,
      items: cur,
      running: tools.some((i) => i.pending),
      failures: tools.filter((i) => i.ok === false).length,
      toolCount: tools.length,
      startTs: tsList.length > 0 ? Math.min(...tsList) : undefined,
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
        <p
          v-if="chat.items.length === 0"
          class="mt-16 text-center text-sm text-ink-4"
        >
          {{ t("chat.empty") }}
        </p>

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
                <ToolRow
                  v-else
                  :item="item"
                  :default-expanded="expandLive && item.ts !== undefined"
                />
              </template>
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
