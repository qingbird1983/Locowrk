<script setup lang="ts">
import { nextTick, ref, watch } from "vue";
import { useChatStore } from "../stores/chat";

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
</script>

<template>
  <main class="flex-1 overflow-y-auto px-5 py-4">
    <div class="mx-auto flex max-w-3xl flex-col gap-3">
      <p
        v-if="chat.items.length === 0"
        class="mt-16 text-center text-sm text-neutral-500"
      >
        发送任意消息开始对话；消息里包含 “echo” 会触发 tool-echo
        插件的工具调用。
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
                item.sideEffect === 'pure'
                  ? 'bg-emerald-900/60 text-emerald-300'
                  : 'bg-red-900/60 text-red-300'
              "
            >
              {{ item.sideEffect }}
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
              {{ item.pending ? "执行中…" : item.ok ? "成功" : "失败" }}
            </span>
          </div>
          <div class="space-y-2 px-3 py-2">
            <div>
              <p class="mb-1 text-[10px] uppercase tracking-wider text-neutral-500">
                参数
              </p>
              <pre
                class="overflow-x-auto rounded bg-neutral-900/80 p-2 font-mono text-[11px] text-neutral-300"
                >{{ pretty(item.args) }}</pre
              >
            </div>
            <div v-if="!item.pending">
              <p class="mb-1 text-[10px] uppercase tracking-wider text-neutral-500">
                结果
              </p>
              <pre
                class="overflow-x-auto rounded bg-neutral-900/80 p-2 font-mono text-[11px]"
                :class="item.ok ? 'text-neutral-300' : 'text-red-300'"
                >{{ item.ok ? pretty(item.output) : item.error }}</pre
              >
            </div>
          </div>
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
