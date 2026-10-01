<script setup lang="ts">
/**
 * 工具调用卡模式（UI shell doc §3.7，㉑）：工具执行过程默认折叠成单行
 * 摘要（箭头 + 工具名 + 徽章 + 参数摘要 + 状态），点击头部展开参数/结果
 * 明细——否则一轮多工具的会话瞬间被 JSON 刷满，正文全被挤走。
 *
 * 折叠状态是组件本地 UI 状态：不入 store、不持久化，切会话/重放回到
 * 折叠；唯一例外是失败自动展开一次（错误可见性优先于密度，§3.7）。
 */
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { clip } from "../../lib/format";
import type { ToolItem } from "../../stores/chat";

const { t } = useI18n();
const props = defineProps<{ item: ToolItem }>();

const expanded = ref(false);

// 失败自动展开一次：实时结果到达（pending 翻转）与历史重放（挂载即失败）
// 同规；用户随后可手动折叠。
watch(
  () => !props.item.pending && props.item.ok === false,
  (failed) => {
    if (failed) expanded.value = true;
  },
  { immediate: true },
);

/** 折叠行参数摘要：args 首个原始值截断（§3.7），取不到就留空。 */
function argSummary(args: unknown): string {
  if (args !== null && typeof args === "object") {
    for (const v of Object.values(args as Record<string, unknown>)) {
      if (typeof v === "string") return clip(v, 48);
      if (typeof v === "number" || typeof v === "boolean") return String(v);
    }
  }
  return "";
}

function pretty(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}
</script>

<template>
  <div
    class="max-w-[85%] overflow-hidden rounded-lg border border-line-strong/70 bg-surface-1/40 text-xs"
  >
    <!-- 头部整行可点：折叠/展开切换（§3.7；hover 过渡由 base 层统管） -->
    <button
      class="flex w-full items-center gap-2 px-3 py-2 text-left hover:bg-surface-2/40"
      :class="expanded ? 'border-b border-line-strong/50' : ''"
      @click="expanded = !expanded"
    >
      <span class="w-3 shrink-0 text-ink-4">{{ expanded ? "▾" : "▸" }}</span>
      <span class="shrink-0 font-mono font-semibold text-amber-400">
        ⚙ {{ item.tool }}
      </span>
      <span
        class="shrink-0 rounded bg-surface-2/60 px-1.5 py-0.5 text-[10px] text-ink-2"
      >
        v{{ item.pluginVersion }}
      </span>
      <span
        class="shrink-0 rounded px-1.5 py-0.5 text-[10px]"
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
      <span class="min-w-0 flex-1 truncate text-ink-4">
        {{ argSummary(item.args) }}
      </span>
      <span
        class="shrink-0"
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
    </button>
    <!-- 明细体：v-show 保 DOM（§3.7），重放与补丁都便宜 -->
    <div v-show="expanded" class="space-y-2 px-3 py-2">
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
</template>
