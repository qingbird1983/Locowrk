<script setup lang="ts">
/**
 * 轮计时读数（台账 AF-14）：图标 + 「工作中 / 已工作 N 分 N 秒」。
 *
 * 只是读数、**不带交互**——壳由两个宿主各自给：容器计时行
 * （`TurnTimerLine`，整行是容器折叠钮）与动作块组头（`TurnProcess`，
 * 行内那颗小按钮是容器折叠钮）。一处算两处用，免得同一个读数出现两种
 * 写法：AF-14 之前容器行写 `0 分 34 秒`、组头同时刻写 `34s`（同一属性
 * 两种形态，正是用户点掉的毛病）。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import { useTurnClock } from "../../lib/use-turn-clock";

const props = defineProps<{
  /** 轮起点 = 用户消息实时时刻；undefined = 无从计时（诚实省略读数）。 */
  startTs?: number;
  /** 轮收束时刻（v2.101，重放读数终点；实时收束走本地 finishedAt）。 */
  endTs?: number;
  /** 本轮真正干活的毫秒数（v2.205，`agent://turn-header`）。undefined =
   * 旧日志读不出 ⇒ 回落到墙钟差；**不得当 0**。 */
  activeMs?: number;
  /** 当轮是否在跑（收束即冻结读数）。 */
  running: boolean;
}>();

const { t, locale } = useI18n();

const { seconds } = useTurnClock({
  startTs: () => props.startTs,
  running: () => props.running,
  endTs: () => props.endTs,
  activeMs: () => props.activeMs,
});

/** 本地化分秒读数（裁定示例「工作中 0 分 1 秒」——分位恒显示）。 */
const text = computed(() => {
  const sec = seconds.value;
  if (sec === null) return "";
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return locale.value === "zh-CN" ? `${m} 分 ${s} 秒` : `${m}m ${s}s`;
});
</script>

<template>
  <span class="turn-readout">
    <Icon
      v-if="startTs !== undefined"
      :name="running ? 'clock' : 'check'"
      :size="12"
      class="shrink-0 text-ink-4"
    />
    <span v-if="text">{{
      running ? t("turn.working", { t: text }) : t("turn.worked", { t: text })
    }}</span>
  </span>
</template>

<style scoped>
/* 只给「一行内排得下」的最小几何：字号/颜色一律继承宿主（容器行 11px、
   组头 13px 各自有语境），别在这里再定一次 */
.turn-readout {
  display: inline-flex;
  align-items: center;
  gap: var(--lw-gap-2);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}
</style>
