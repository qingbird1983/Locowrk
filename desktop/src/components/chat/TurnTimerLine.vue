<script setup lang="ts">
/**
 * 轮计时行（v2.99「第 0 层」用户裁定）：任务容器的常驻头部——随用户
 * 消息开启，「⏱ 工作中 0 分 1 秒」起表；整轮收束读数冻结为「✓ 已工作
 * N 分 N 秒」。v2.101 起轮界时刻落会话日志（TurnStart/TurnEnd），重放
 * 复原读数；旧日志无时刻 = 无读数（只剩发丝线，不虚构时序），调度注入
 * 轮同理（无从起算）。
 *
 * v2.205 改两处：①走表机器不再本文件自写——与 `TurnProcess` 同形的那套
 * `setInterval` + 收束戳移到 `useTurnClock`（同一属性只许一个出处）；
 * ②收束读数优先用宿主的 `activeMs`（**墙钟区间扣掉等待用户审批的那一段**），
 * 拿不到才回落墙钟差。文案本就写着「已工作」，扣掉等待才是它说的这件事。
 *
 * AF-14 终版（用户 2026-10-19 裁定「保留 ✓ + 时长那条，删掉组头那层」）：本行是
 * **一轮唯一的头行**——动作块组头整层已删（`TurnProcess.vue` 只剩组体），组头原先
 * 那几个标记随之下沉到本行：走表读数（`TurnTimerReadout`，与旧组头那枚 `34s` 共用
 * 一份实现）、「已停止」（原组头标签态）、「N 个问题」（原组头信息位）。整行可点 =
 * 折叠 / 展开整个轮容器。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import TurnTimerReadout from "./TurnTimerReadout.vue";

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
  /** 本轮被用户停止（原组头「已停止」；重放与未停止轮恒否）。 */
  stopped?: boolean;
  /** 轮内失败的工具调用数（原组头「N 个问题」；0 / 缺省 = 不渲染）。 */
  problems?: number;
}>();
const emit = defineEmits<{ click: [] }>();
const { t } = useI18n();

/** 读数在场才给标记前面那个分隔点——无从计时的轮只剩发丝线，不该以「·」开头。 */
const hasReading = computed(() => props.startTs !== undefined);
</script>

<template>
  <button class="turn-timer" type="button" @click="emit('click')">
    <TurnTimerReadout
      :start-ts="startTs"
      :end-ts="endTs"
      :active-ms="activeMs"
      :running="running"
    />
    <span v-if="stopped" class="turn-timer__mark">
      <span v-if="hasReading" class="turn-timer__sep" aria-hidden="true"
        >·</span
      >
      {{ t("turn.stopped") }}
    </span>
    <span v-if="problems" class="turn-timer__mark">
      <span v-if="hasReading || stopped" class="turn-timer__sep" aria-hidden="true"
        >·</span
      >
      <Icon name="info" :size="12" />
      {{ t("turn.problemCount", { n: problems }) }}
    </span>
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
.turn-timer:hover {
  color: var(--lw-ink-2);
}
/* 标记（已停止 / N 个问题）：与读数同色同字号，靠分隔点接在读数后面——
 * 行内 gap 只有 2px（那是图标↔文字的量），给分隔点补 4px 凑成 `.prow` 同族的 6px。 */
.turn-timer__mark {
  display: inline-flex;
  align-items: center;
  gap: var(--lw-gap-4);
}
.turn-timer__sep {
  margin-left: var(--lw-gap-4);
  color: var(--lw-ink-4);
}
</style>
