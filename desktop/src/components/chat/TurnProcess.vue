<script setup lang="ts">
/**
 * 动作块（UI shell doc §3.7 + 四层折叠 v2.96→v2.99 修订）——任务容器内
 * 的块级单元，三种形态（用户裁定「层层折叠」）：
 *   L3 = 正在执行：完全展开、流式显示（无组头，chrome 由容器计时行承担；
 *        思考行流式即展开，ThinkingRow streaming 驱动）；
 *   L2 = 动作完成且模型出简短回复：半折叠为一行摘要（sparkle + 已完成
 *        + · 耗时 + · N 个工具）；
 *   L1 = 下一个动作开始 / 整轮收束：全折叠为一行极简摘要（动作名串联，
 *        失败时附 · N 个问题）。
 * 容器整体（计时行 + 全部动作块 + 简短回复）在 ChatTranscript 的
 * .turn-box：收束后收缩为「✓ 已工作 + 发丝线」一行，点击逐层重开。
 * 相位由父层给（live / replied / done），相位变化全映射落层（live→2，
 * 其余→1，不搞边沿配对——running 瞬时翻假后层级必须能自愈，v2.97 教训）。
 * 折叠是本地 UI 状态：不入 store、不持久化。「默认展开工具执行过程」
 * 开关 = 非执行块的挂载初值。时长只在实时轮次有（历史重放无时间戳，
 * 不虚构时序）。
 */
import { computed, onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import { clip } from "../../lib/format";

const props = withDefaults(
  defineProps<{
    /** v4.11 §3.39④：本轮被用户停止（收束后头行「已停止」，重放恒否）。 */
    stopped?: boolean;
    toolCount: number;
    failures: number;
    /** L1 极简摘要：块内动作名（去重保序，父层算好）。 */
    names?: string[];
    /** 组内最早的过程项时刻；undefined = 历史重放（不显示时长）。 */
    startTs?: number;
    /** 块相位（父层按四层状态机计算）：live / replied / done。 */
    phase: "live" | "replied" | "done";
    defaultExpanded?: boolean;
  }>(),
  { phase: "done" },
);

const { t } = useI18n();

/** 执行中 = live 相位（父层状态机唯一真源）：组体常开、无组头，时长
 * 走表也吃它——动作间隙相位不变，走表不被间隙打断冻结（v2.98）。 */
const running = computed(() => props.phase === "live");

/** 当前层：1（组头）/ 2（组体展开）。L0 在 TurnTimerLine（v2.99 迁出）。 */
const level = ref<[1, 2][number]>(initialLevel());

function initialLevel(): 1 | 2 {
  if (props.phase === "live") return 2;
  return props.defaultExpanded ? 2 : 1;
}

// 相位变化 = 全映射落层：live→2（组体常开）/ 其余→1（折为组头）。不搞
// 边沿配对——running 在轮界/重建时会瞬时翻假，全员先被打回折叠态，随后
// 的翻转若依赖「prev===live」之类配对就永远卡死（v2.97 白屏教训）；任何
// 相位变化都直接落到规格层，层级自愈。
watch(
  () => props.phase,
  (p) => {
    level.value = p === "live" ? 2 : 1;
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

// 执行中的头行时长每秒走字（收束停在收束时刻）。挂载边沿不造收束戳
// （重放无 ts 本就无时长）；startTs 晚到也要起表；相位瞬时翻假后回 live
// 清戳恢复走表，真实收束（不回 live）戳保持。
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

/** L1 极简摘要（裁定示例「读取 + 分析」）：动作名串联，超长截断。 */
const minimalText = computed(() =>
  props.names && props.names.length > 0
    ? clip(props.names.join(" + "), 32)
    : t("turn.done"),
);
</script>

<template>
  <div class="turn" :class="{ 'is-open': level === 2 }">
    <!-- L2 摘要行 / L1 极简行（live 无此行——组体全展开，chrome 在容器
         计时行）；点击开合组体 -->
    <button
      v-if="phase !== 'live'"
      class="cv-head"
      type="button"
      :aria-expanded="level === 2"
      @click="onToggle"
    >
      <Icon name="sparkle" :size="14" class="shrink-0 text-ink-1" />
      <span class="cv-head__label">
        {{
          stopped
            ? t("turn.stopped")
            : phase === "done"
              ? minimalText
              : t("turn.done")
        }}
      </span>
      <span v-if="phase === 'replied'" class="cv-head__meta">{{
        metaText
      }}</span>
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
  </div>
</template>

<style scoped>
.turn {
  /* v4.14 三层间距统一 8px：父容器 flex+gap 给间距（层 1 头行 ↔ 层 2 组体），
   * 不用 margin；垂直 padding 全零，跨组件兄弟间距在 TranscriptFlow。
   * gap 只在展开态给——折叠态 grid 收为 0 高，常驻 gap 会留下 8px 死空间
   * 垫在头行下，把「折叠层↔简短回复 8px」撑成 16px（2026-10-07 用户裁定
   * 收紧）；收/展瞬间 gap 8px 直落/直起，滑体动画不受影响 */
  display: flex;
  flex-direction: column;
  gap: 0;
  padding: 0;
}
.turn.is-open {
  gap: var(--lw-gap-8);
}
/* v4.14 第 1 层主色：头行整线 ink-1（字形/标签/meta/三角同色） */
.turn .cv-head {
  color: var(--lw-ink-1);
}
.turn .cv-head__meta {
  color: var(--lw-ink-1);
}
.turn .cv-head__chev {
  color: var(--lw-ink-1);
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
   * 不缩进、无引导线，扁平直排；v4.14 行距统一 8px（层 2 步骤项之间） */
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-8);
  transition: opacity var(--dur-fast) var(--ease-in);
}
.turn__grid.is-open .turn__body {
  opacity: 1;
  transition: opacity var(--dur-base) var(--ease-out) 40ms;
}
/* 嵌套折叠纪律（§6.6 ④）：一次只动被操作的层——内部子单元各自只在被
 * 直接点击时才动画（v-show / 各组件自己的 0fr），此处不加二次动画。 */
</style>
