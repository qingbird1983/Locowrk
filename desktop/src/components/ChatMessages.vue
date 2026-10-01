<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { contextUndoCompact } from "../lib/api";
import { useChatStore, type SystemItem } from "../stores/chat";
import SystemNotice from "./chat/SystemNotice.vue";
import ToolCallCard from "./chat/ToolCallCard.vue";

const { t } = useI18n();
const chat = useChatStore();
const bottom = ref<HTMLElement | null>(null);

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

/** 系统消息动作（⑨ 接线）：details = 展开详情；undo = 撤销最近一次压缩
 * （rewind_to 回到压缩前，其后的轮次一并回退，会话 id 不变）。 */
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
  <main class="flex-1 overflow-y-auto px-5 py-4">
    <div class="mx-auto flex max-w-3xl flex-col gap-3">
      <p
        v-if="chat.items.length === 0"
        class="mt-16 text-center text-sm text-ink-4"
      >
        {{ t("chat.empty") }}
      </p>

      <template v-for="item in chat.items" :key="item.id">
        <!-- 用户消息：右侧气泡（wash 分层；过渡形态，㉔ MessageRow 重写） -->
        <div v-if="item.kind === 'user'" class="flex justify-end">
          <div
            class="max-w-[75%] whitespace-pre-wrap rounded-2xl rounded-br-sm bg-wash-3 px-4 py-2 text-sm"
          >
            {{ item.text }}
          </div>
        </div>

        <!-- 助手消息：左侧气泡 -->
        <div v-else-if="item.kind === 'assistant'" class="flex justify-start">
          <div
            class="max-w-[75%] whitespace-pre-wrap rounded-2xl rounded-bl-sm bg-wash-1 px-4 py-2 text-sm"
          >
            {{ item.text }}
          </div>
        </div>

        <!-- 工具调用卡（§3.7 ToolCallCard 模式，㉑）：默认折叠单行摘要 -->
        <ToolCallCard v-else-if="item.kind === 'tool'" :item="item" />

        <!-- 会话内系统消息（SystemNotice 模式）：压缩通知/已记住/审批结果 -->
        <div v-else-if="item.kind === 'system'" class="flex justify-center">
          <SystemNotice
            :text="noticeText(item)"
            :actions="
              (item.actions ?? []).map((a) => ({
                id: a,
                label: t(`notice.action.${a}`),
              }))
            "
            @action="(a) => onNoticeAction(item, a)"
          />
        </div>

        <!-- 错误条 -->
        <div
          v-else
          class="rounded-lg bg-danger/10 px-3 py-2 text-xs text-danger"
        >
          {{ item.text }}
        </div>
      </template>
      <div ref="bottom" />
    </div>
  </main>
</template>
