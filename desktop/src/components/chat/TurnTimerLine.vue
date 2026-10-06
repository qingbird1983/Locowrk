<script setup lang="ts">
/**
 * 轮计时行（v2.99「第 0 层」用户裁定）：任务容器的常驻头部——随用户
 * 消息开启，「⏱ 工作中 0 分 1 秒」起表；整轮收束读数冻结为「✓ 已工作
 * N 分 N 秒」且容器收缩后它是唯一残留（点击逐层重开），发丝线在文字
 * 下方。v2.101 起轮界时刻落会话日志（TurnStart/TurnEnd），重放复原
 * 读数；旧日志无时刻 = 无读数（只剩发丝线，不虚构时序），调度注入轮
 * 同理（无从起算）。
 */
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";

const props = defineProps<{
  /** 轮起点 = 用户消息实时时刻；undefined = 无从计时（诚实省略读数）。 */
  startTs?: number;
  /** 轮收束时刻（v2.101，重放读数终点；实时收束走本地 finishedAt）。 */
  endTs?: number;
  /** 当轮是否在跑（收束即冻结读数）。 */
  running: boolean;
}>();
const emit = defineEmits<{ click: [] }>();
const { t, locale } = useI18n();

const now = ref(Date.now());
const finishedAt = ref<number | null>(null);
let timer: number | null = null;
watch(
  () => props.running,
  (r, prev) => {
    if (r) {
      // 瞬时翻假回 live（v2.97 实抓）清戳恢复走表；真实收束不回 live
      if (prev !== undefined) finishedAt.value = null;
      if (timer === null) {
        now.value = Date.now();
        timer = window.setInterval(() => (now.value = Date.now()), 1000);
      }
    } else {
      if (prev !== undefined && prev && finishedAt.value === null) {
        finishedAt.value = Date.now();
      }
      if (timer !== null) {
        window.clearInterval(timer);
        timer = null;
      }
    }
  },
  { immediate: true },
);
onBeforeUnmount(() => {
  if (timer !== null) window.clearInterval(timer);
});

/** 本地化分秒读数（裁定示例「工作中 0 分 1 秒」——分位恒显示）。
 * 终点优先级：本地收束戳（实时）> 重放 endTs（v2.101 轮界事件）> 走表。 */
const text = computed(() => {
  if (props.startTs === undefined) return "";
  const end =
    finishedAt.value ?? props.endTs ?? (props.running ? now.value : null);
  if (end === null) return "";
  const sec = Math.max(0, Math.round((end - props.startTs) / 1000));
  const m = Math.floor(sec / 60);
  const s = sec % 60;
  return locale.value === "zh-CN" ? `${m} 分 ${s} 秒` : `${m}m ${s}s`;
});
</script>

<template>
  <button class="turn-timer" type="button" @click="emit('click')">
    <Icon
      v-if="startTs !== undefined"
      :name="running ? 'clock' : 'check'"
      :size="12"
      class="shrink-0 text-ink-4"
    />
    <span v-if="text">{{
      running ? t("turn.working", { t: text }) : t("turn.worked", { t: text })
    }}</span>
  </button>
</template>

<style scoped>
.turn-timer {
  display: flex;
  width: 100%;
  align-items: center;
  gap: var(--lw-gap-2);
  padding: var(--lw-gap-2) 0 var(--lw-gap-6);
  border-bottom: 1px solid var(--lw-line-subtle);
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-3);
  font-variant-numeric: tabular-nums;
  text-align: left;
}
.turn-timer:hover span {
  color: var(--lw-ink-2);
}
</style>
