<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { contextUndoCompact } from "../lib/api";
import { useChatStore, type SystemItem } from "../stores/chat";
import SystemNotice from "./chat/SystemNotice.vue";

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

function pretty(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}

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
        <!-- 用户消息：右侧气泡 -->
        <div v-if="item.kind === 'user'" class="flex justify-end">
          <div
            class="max-w-[75%] whitespace-pre-wrap rounded-2xl rounded-br-sm bg-emerald-700/80 px-4 py-2 text-sm"
          >
            {{ item.text }}
          </div>
        </div>

        <!-- 助手消息：左侧气泡 -->
        <div v-else-if="item.kind === 'assistant'" class="flex justify-start">
          <div
            class="max-w-[75%] whitespace-pre-wrap rounded-2xl rounded-bl-sm bg-surface-1 px-4 py-2 text-sm"
          >
            {{ item.text }}
          </div>
        </div>

        <!-- 工具调用卡片（beautifului Tool Chips 形态的 Tailwind 实现） -->
        <div
          v-else-if="item.kind === 'tool'"
          class="max-w-[85%] rounded-lg border border-line-strong/70 bg-surface-1/40 text-xs"
        >
          <div
            class="flex items-center gap-2 border-b border-line-strong/50 px-3 py-2"
          >
            <span class="font-mono font-semibold text-amber-400">
              ⚙ {{ item.tool }}
            </span>
            <span
              class="rounded bg-surface-2/60 px-1.5 py-0.5 text-[10px] text-ink-2"
            >
              v{{ item.pluginVersion }}
            </span>
            <span
              class="rounded px-1.5 py-0.5 text-[10px]"
              :class="
                item.sideEffect === 'irreversible'
                  ? 'bg-red-900/60 text-red-300'
                  : item.sideEffect === 'compensable'
                    ? 'bg-amber-900/60 text-amber-300'
                    : 'bg-emerald-900/60 text-emerald-300'
              "
            >
              {{ t(`effect.${item.sideEffect}`) }}
            </span>
            <span
              class="ml-auto"
              :class="
                item.pending
                  ? 'text-ink-4'
                  : item.ok
                    ? 'text-emerald-400'
                    : 'text-red-400'
              "
            >
              {{
                item.pending
                  ? t("tool.running")
                  : item.ok
                    ? t("tool.success")
                    : t("tool.failed")
              }}
            </span>
          </div>
          <div class="space-y-2 px-3 py-2">
            <div>
              <p class="mb-1 text-[10px] uppercase tracking-wider text-ink-4">
                {{ t("tool.params") }}
              </p>
              <pre
                class="overflow-x-auto rounded bg-surface-0/80 p-2 font-mono text-[11px] text-ink-2"
                >{{ pretty(item.args) }}</pre
              >
            </div>
            <div v-if="!item.pending">
              <p class="mb-1 text-[10px] uppercase tracking-wider text-ink-4">
                {{ t("tool.result") }}
              </p>
              <pre
                class="overflow-x-auto rounded bg-surface-0/80 p-2 font-mono text-[11px]"
                :class="item.ok ? 'text-ink-2' : 'text-red-300'"
                >{{ item.ok ? pretty(item.output) : item.error }}</pre
              >
            </div>
          </div>
        </div>

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
          class="rounded-lg border border-red-800/60 bg-red-950/50 px-3 py-2 text-xs text-red-300"
        >
          {{ item.text }}
        </div>
      </template>
      <div ref="bottom" />
    </div>
  </main>
</template>
