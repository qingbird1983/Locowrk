<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../stores/chat";
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

/** 系统消息动作（details/undo）：实际行为 ⑨ 接线（rewind_to / 详情）。 */
function onNoticeAction(_id: string) {}
</script>

<template>
  <main class="flex-1 overflow-y-auto px-5 py-4">
    <div class="mx-auto flex max-w-3xl flex-col gap-3">
      <p
        v-if="chat.items.length === 0"
        class="mt-16 text-center text-sm text-neutral-500"
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
            class="max-w-[75%] whitespace-pre-wrap rounded-2xl rounded-bl-sm bg-neutral-800 px-4 py-2 text-sm"
          >
            {{ item.text }}
          </div>
        </div>

        <!-- 工具调用卡片（beautifului Tool Chips 形态的 Tailwind 实现） -->
        <div
          v-else-if="item.kind === 'tool'"
          class="max-w-[85%] rounded-lg border border-neutral-700/70 bg-neutral-800/40 text-xs"
        >
          <div
            class="flex items-center gap-2 border-b border-neutral-700/50 px-3 py-2"
          >
            <span class="font-mono font-semibold text-amber-400">
              ⚙ {{ item.tool }}
            </span>
            <span
              class="rounded bg-neutral-700/60 px-1.5 py-0.5 text-[10px] text-neutral-300"
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
                  ? 'text-neutral-500'
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
              <p class="mb-1 text-[10px] uppercase tracking-wider text-neutral-500">
                {{ t("tool.params") }}
              </p>
              <pre
                class="overflow-x-auto rounded bg-neutral-900/80 p-2 font-mono text-[11px] text-neutral-300"
                >{{ pretty(item.args) }}</pre
              >
            </div>
            <div v-if="!item.pending">
              <p class="mb-1 text-[10px] uppercase tracking-wider text-neutral-500">
                {{ t("tool.result") }}
              </p>
              <pre
                class="overflow-x-auto rounded bg-neutral-900/80 p-2 font-mono text-[11px]"
                :class="item.ok ? 'text-neutral-300' : 'text-red-300'"
                >{{ item.ok ? pretty(item.output) : item.error }}</pre
              >
            </div>
          </div>
        </div>

        <!-- 会话内系统消息（SystemNotice 模式）：压缩通知/已记住/审批结果 -->
        <div v-else-if="item.kind === 'system'" class="flex justify-center">
          <SystemNotice
            :text="t(item.key, item.params ?? {})"
            :actions="
              (item.actions ?? []).map((a) => ({
                id: a,
                label: t(`notice.action.${a}`),
              }))
            "
            @action="onNoticeAction"
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
