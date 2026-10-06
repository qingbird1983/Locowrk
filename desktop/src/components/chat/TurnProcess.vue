<script setup lang="ts">
/**
 * 轮次过程组（UI shell doc §3.7 + v2.96 四层折叠状态机，v2.98 修订）：
 * 一轮动作块有四层形态——
 *   L3 = 单个动作的展开明细（ToolRow，只由用户点开——v2.98 裁定「永远
 *        不要替用户展开 3 层」，运行中与失败都不自动展开）；
 *   L2 = 组体展开、动作各折一行（本组件 grid-rows 开合）；
 *   L1 = 组头行（sparkle + 状态词 + · 时长 + · N 个工具 + chevron）；
 *   L0 = 整轮收束：无简介内容，只留耗时一行 + 发丝线分割（历史重放无
 *        时间戳 = 只有发丝线，诚实呈现）；失败不加任何标记（v2.98 裁定
 *        去脏标——整轮耗时足够）。
 * 相位由父层给（live 动作执行中 / replied 模型回话中 / done 整轮收束），
 * 相位变化全映射落层（live→2 / replied→1 / done→0，不搞边沿配对——
 * running 瞬时翻假后层级必须能自愈，见下方 watcher 注释）；live 相位
 * 贯穿动作间隙（v2.98 裁定：折叠只由「模型回复出现」触发）。用户点击
 * 仍可逐层展开（L0 行 → L1 头 → L2 体 → 单动作 L3）。
 * 折叠是本地 UI 状态：不入 store、不持久化。「默认展开工具执行过程」
 * 开关 = done 组挂载初值（L0 / L1），运行相位不吃开关。时长只在实时
 * 轮次有（历史重放无时间戳，不虚构时序）。
 */
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";

const props = withDefaults(
  defineProps<{
    /** v4.11 §3.39④：本轮被用户停止（收束后头行「已停止」，重放恒否）。 */
    stopped?: boolean;
    toolCount: number;
    failures: number;
    /** 组内最早的过程项时刻；undefined = 历史重放（不显示时长）。 */
    startTs?: number;
    /** 组相位（父层按四层状态机计算）：live / replied / done。 */
    phase: "live" | "replied" | "done";
    defaultExpanded?: boolean;
  }>(),
  { phase: "done" },
);

const { t } = useI18n();

/** 执行中 = live 相位（父层状态机唯一真源，v2.98 起不再单传 running）：
 * 头行「执行中」与时长走表都吃它——动作间隙相位不变，时长不再被间隙
 * 打断冻结。 */
const running = computed(() => props.phase === "live");

/** 当前层：0（耗时行+发丝线）/ 1（组头）/ 2（组体展开）。L3 在 ToolRow。 */
const level = ref<[0, 1, 2][number]>(initialLevel());

function initialLevel(): 0 | 1 | 2 {
  if (props.phase === "live") return 2;
  if (props.phase === "replied") return 1;
  return props.defaultExpanded ? 1 : 0;
}

// 相位变化 = 全映射落层（不搞边沿配对）：live→2 / replied→1 / done→0。
// 走查实抓（v2.97 白屏）：running 在轮界/重建时会瞬时翻假，全员先被打
// 回 done（L0 发丝线），随后的 done→replied/live 若依赖「prev===live」
// 之类边沿配对就永远卡在 L0——任务还在跑、转录却只剩发丝线 = 用户眼中
// 的白屏。任何相位变化都直接落到规格层，层级自愈；用户手动开合只在相
// 位稳定期间有效，下次相位变化再归位。
watch(
  () => props.phase,
  (p) => {
    level.value = p === "live" ? 2 : p === "replied" ? 1 : 0;
  },
);

function onToggle() {
  level.value = level.value === 2 ? 1 : 2;
}

/** 展开动画结束后才摘 overflow（否则聚焦环/引导线被裁）。 */
const settled = ref(false);
function onTransitionEnd(e: TransitionEvent) {
  if (e.propertyName !== "grid-template-rows") return;
  if (level.value === 2) settled.value = true;
}

// 运行中的头行时长每秒走字（运行结束停在收束时刻）。挂载边沿不造收束
// 戳（重放无 ts 本就无时长；实时已收束组重挂诚实省略）；startTs 晚到
// （思考先行、工具后到）也要起表；相位瞬时翻假后回 live（v2.97 实抓
// running 会瞬时翻假）清戳恢复走表，真实收束（done 后不回 live）戳保持。
const now = ref(Date.now());
const finishedAt = ref<number | null>(null);
let timer: number | null = null;
watch(
  [running, () => props.startTs],
  ([r, ts], prev) => {
    if (r && ts !== undefined) {
      if (prev !== undefined) finishedAt.value = null;
      if (timer === null) {
        now.value = Date.now();
        timer = window.setInterval(() => (now.value = Date.now()), 1000);
      }
    } else {
      if (prev !== undefined && prev[0] && finishedAt.value === null) {
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

const durationText = computed(() => {
  if (props.startTs === undefined) return "";
  const end = finishedAt.value ?? (running.value ? now.value : null);
  if (end === null) return "";
  const sec = Math.max(0, Math.round((end - props.startTs) / 1000));
  if (sec < 60) return `${sec}s`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m ${sec % 60}s`;
  return `${Math.floor(sec / 3600)}h ${Math.floor((sec % 3600) / 60)}m`;
});

const metaText = computed(() => {
  const parts = [
    durationText.value,
    t("turn.toolCount", { n: props.toolCount }),
  ].filter(Boolean);
  return `· ${parts.join(" · ")}`;
});
</script>

<template>
  <div class="turn">
    <!-- L0：整轮收束——只留耗时统计，下发丝线分割；点击展开到 L1。
         历史重放无时间戳 = 行内无字（诚实呈现），发丝线仍可点；
         失败不加任何标记（v2.98 裁定去脏标，耗时足够） -->
    <button
      v-if="level === 0"
      class="turn-zero"
      type="button"
      :aria-expanded="false"
      @click="level = 1"
    >
      <span v-if="durationText" class="turn-zero__dur">{{ durationText }}</span>
    </button>
    <template v-else>
      <button
        class="cv-head"
        type="button"
        :aria-expanded="level === 2"
        @click="onToggle"
      >
        <Icon name="sparkle" :size="14" class="shrink-0 text-ink-3" />
        <span class="cv-head__label">
          {{ running ? t("turn.running") : stopped ? t("turn.stopped") : t("turn.done") }}
        </span>
        <span class="cv-head__meta">{{ metaText }}</span>
        <span
          v-if="failures > 0"
          class="cv-head__meta inline-flex items-center gap-1"
        >
          <Icon name="info" :size="12" />
          {{ t("turn.problemCount", { n: failures }) }}
        </span>
        <Icon
          :name="level === 2 ? 'chevronDown' : 'chevronRight'"
          :size="12"
          class="cv-head__chev"
        />
      </button>
      <!-- §6.6 ④：grid-rows 0fr→1fr 滑动；内容 opacity 展开延后 40ms -->
      <div
        class="turn__grid"
        :class="{ 'is-open': level === 2, 'is-settled': settled }"
        @transitionend="onTransitionEnd"
      >
        <div class="turn__body"><slot /></div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.turn {
  padding: var(--lw-gap-4) 0 var(--lw-gap-6);
}
/* L0 行：整行点击热区；有字显字、无字只剩发丝线（行高由线距兜住） */
.turn-zero {
  display: flex;
  width: 100%;
  align-items: center;
  gap: var(--lw-gap-2);
  padding: var(--lw-gap-2) 0 var(--lw-gap-6);
  border-bottom: 1px solid var(--lw-line-subtle);
  text-align: left;
}
.turn-zero__dur {
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-3);
  font-variant-numeric: tabular-nums;
}
.turn-zero:hover .turn-zero__dur {
  color: var(--lw-ink-2);
}
.turn__grid {
  display: grid;
  /* v2.74·fix（目检实抓，CDP 实测 body 1693 vs 容器 560）：隐式 auto 列轨
   * 按内容 max-content 撑轨越界——列宽必须显式钉死容器；min-width:0 兜
   * settled 态 overflow:visible 的自动最小尺寸。 */
  grid-template-columns: minmax(0, 1fr);
  grid-template-rows: 0fr;
  transition: grid-template-rows var(--dur-fast) var(--ease-in);
}
.turn__grid.is-open {
  grid-template-rows: 1fr;
  transition: grid-template-rows var(--dur-base) var(--ease-slide);
}
.turn__grid.is-settled .turn__body {
  overflow: visible;
}
.turn__body {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  opacity: 0;
  /* v4.11 用户裁定（照 ZCode 过程流基准）：展开体与头行同一左缘——
   * 不缩进、无引导线，扁平直排 */
  display: flex;
  flex-direction: column;
  gap: 1px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值1,§二⑬) */
  margin-top: var(--lw-gap-2);
  transition: opacity var(--dur-fast) var(--ease-in);
}
.turn__grid.is-open .turn__body {
  opacity: 1;
  transition: opacity var(--dur-base) var(--ease-out) 40ms;
}
/* 嵌套折叠纪律（§6.6 ④）：一次只动被操作的层——内部子单元各自只在被
 * 直接点击时才动画（v-show / 各组件自己的 0fr），此处不加二次动画。 */
</style>
