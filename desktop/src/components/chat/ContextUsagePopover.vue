<script setup lang="ts">
/**
 * 上下文用量浮层（UI shell doc §3.12）：锚在用量环按钮上方、右对齐，
 * 360px elevated popover。分项读数照参考对象——后四项（本轮合计 / 生成
 * 速度 / 模型用量 / 工具上下文）需要后端扩展载荷，§8.2 记录待补，这里只
 * 出已有读数（used / threshold / windowTokens）。headline 口径跟设置
 * usageReadout（已用 / 剩余）。数字一律过 §3.5 compactNumber。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { compactNumber } from "../../lib/format";

const props = defineProps<{
  usage: { used: number; threshold: number; windowTokens: number };
  readout: string;
}>();
defineEmits<{ close: [] }>();

const { t } = useI18n();

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
.kv__v {
  color: var(--lw-ink-1);
  font-variant-numeric: tabular-nums;
}
</style>
