<script setup lang="ts">
/**
 * 上下文用量浮层（UI shell doc §3.12）：锚在用量环按钮上方、右对齐，
 * 360px elevated popover。分项读数照参考对象——v2.58 数据源落地：
 * 「本轮合计 / 生成速度 / 模型用量（输入/输出/缓存读取/命中率/推理/写入）」
 * 来自 agent://usage 按轮聚合（chat.turnUsage，供应商不报告 = 行整体不
 * 渲染）；「工具上下文」= 本轮调用种数/次数（chat items 自带）+ 暴露
 * 目录估算（context-usage 载荷 toolsEstTokens，0 = 目录缺席不渲染）。
 * headline 口径跟设置 usageReadout（已用 / 剩余）。数字一律过 §3.5。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { compactNumber } from "../../lib/format";
import { useChatStore, type ToolItem } from "../../stores/chat";

const props = defineProps<{
  usage: {
    used: number;
    threshold: number;
    windowTokens: number;
    toolsEstTokens: number;
  };
  readout: string;
}>();
defineEmits<{ close: [] }>();

const { t } = useI18n();
const chat = useChatStore();

const pct = computed(() =>
  props.usage.windowTokens > 0
    ? Math.round((props.usage.used / props.usage.windowTokens) * 100)
    : null,
);

const headline = computed(() => {
  if (props.readout === "remaining" && props.usage.windowTokens > 0) {
    const remain = Math.max(0, props.usage.windowTokens - props.usage.used);
    return t("usage.headlineRemaining", { n: compactNumber(remain) });
  }
  return t("usage.headlineUsed", { n: compactNumber(props.usage.used) });
});

const windowText = computed(() => {
  if (props.usage.windowTokens <= 0) {
    return `${compactNumber(props.usage.used)} tokens · ${t("usage.noWindow")}`;
  }
  return `${compactNumber(props.usage.used)} / ${compactNumber(props.usage.windowTokens)} tokens`;
});

/** 本轮合计：优先供应商报告 total（0 = 未报告回退输入+输出+写入）。 */
const turnTotal = computed(() => {
  const u = chat.turnUsage;
  if (!u) return null;
  return u.total > 0 ? u.total : u.input + u.output + u.cacheWrite;
});

/** 缓存命中率 = cache_read / 输入（OpenAI 兼容口径 cached ⊆ prompt）。 */
const cacheHitRate = computed(() => {
  const u = chat.turnUsage;
  if (!u || u.input <= 0 || u.cacheRead <= 0) return null;
  return Math.round((u.cacheRead / u.input) * 100);
});

/** 本轮工具上下文：种数 / 次数（chat items）+ 目录估算（后端载荷）。 */
const toolsText = computed(() => {
  const calls = chat.items.filter(
    (i): i is ToolItem => i.kind === "tool",
  );
  if (calls.length === 0) return null;
  const kinds = new Set(calls.map((c) => c.tool)).size;
  const est = props.usage.toolsEstTokens;
  return (
    t("usage.toolsKindsCalls", { kinds, calls: calls.length }) +
    (est > 0 ? ` · ${t("usage.toolsEst", { n: compactNumber(est) })}` : "")
  );
});
</script>

<template>
  <div class="ctx-popover" role="dialog" :aria-label="t('usage.window')">
    <div class="ctx-popover__head">
      <span>{{ headline }}</span>
      <span v-if="pct !== null" class="kv__v">{{ pct }}%</span>
    </div>
    <div class="kv">
      <span>{{ t("usage.window") }}</span>
      <span class="kv__v">{{ windowText }}</span>
      <span v-if="pct !== null" class="kv__v">{{ pct }}%</span>
    </div>

    <!-- 本轮合计 / 生成速度（§3.12；供应商报告了用量才渲染） -->
    <template v-if="chat.turnUsage">
      <div class="kv">
        <span>{{ t("usage.turnTotal") }}</span>
        <span v-if="turnTotal !== null" class="kv__v">
          {{ compactNumber(turnTotal) }} tokens
        </span>
      </div>
      <div v-if="chat.turnUsage.speed !== null" class="kv">
        <span>{{ t("usage.speed") }}</span>
        <span class="kv__v">{{ t("usage.speedValue", { n: chat.turnUsage.speed }) }}</span>
      </div>
      <!-- 模型用量六类（§3.12 行形态；命中率含在行内） -->
      <div class="kv kv--stack">
        <span>{{ t("usage.modelUsage") }}</span>
        <span class="kv__usage">
          <span>{{ t("usage.input") }} {{ compactNumber(chat.turnUsage.input) }}</span>
          <span>{{ t("usage.output") }} {{ compactNumber(chat.turnUsage.output) }}</span>
          <span v-if="chat.turnUsage.cacheRead > 0">
            {{ t("usage.cacheRead") }} {{ compactNumber(chat.turnUsage.cacheRead) }}
          </span>
          <span v-if="cacheHitRate !== null">{{ t("usage.cacheHit") }} {{ cacheHitRate }}%</span>
          <span v-if="chat.turnUsage.reasoning > 0">
            {{ t("usage.reasoning") }} {{ compactNumber(chat.turnUsage.reasoning) }}
          </span>
          <span v-if="chat.turnUsage.cacheWrite > 0">
            {{ t("usage.cacheWrite") }} {{ compactNumber(chat.turnUsage.cacheWrite) }}
          </span>
        </span>
      </div>
    </template>

    <!-- 工具上下文（本轮调用种数/次数 + 目录估算） -->
    <div v-if="toolsText" class="kv">
      <span>{{ t("usage.toolsContext") }}</span>
      <span class="kv__v">{{ toolsText }}</span>
    </div>
  </div>
</template>

<style scoped>
.ctx-popover {
  position: absolute;
  right: 8px;
  bottom: calc(100% + 10px);
  width: 360px;
  max-width: calc(100vw - 48px);
  border-radius: var(--lw-r-md);
  border: 1px solid var(--lw-line);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-dialog);
  padding: 12px 14px;
  z-index: 30;
  font-size: 12.5px;
  line-height: 1.45;
}
.ctx-popover__head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-weight: 500;
  color: var(--lw-ink-1);
}
.kv {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  margin-top: 8px;
  padding-top: 7px;
  border-top: 1px solid var(--lw-line-subtle);
  color: var(--lw-ink-3);
}
.kv--stack {
  flex-direction: column;
  align-items: flex-start;
  gap: 4px;
}
.kv__usage {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 12px;
  color: var(--lw-ink-1);
  font-variant-numeric: tabular-nums;
}
.kv__v {
  color: var(--lw-ink-1);
  font-variant-numeric: tabular-nums;
}
</style>
