<script setup lang="ts">
/**
 * 动作块（UI shell doc §3.7 + 四层折叠 v2.96→v2.99 修订）——任务容器内
 * 的块级单元，三种形态（用户裁定「层层折叠」）：
 *   L3 = 正在执行：完全展开、流式显示（无组头，chrome 由容器计时行承担；
 *        思考行流式即展开，ThinkingRow streaming 驱动）；
 *   L2 = 动作完成且模型出简短回复：半折叠为一行摘要（sparkle + 已完成
 *        + 轮计时读数 + · N 个工具）；
 *   L1 = 下一个动作开始 / 整轮收束：全折叠为一行极简摘要（动作名串联，
 *        失败时附 · N 个问题）。
 * 容器整体（计时行 + 全部动作块 + 简短回复）在 ChatTranscript 的
 * .turn-box：收束后收缩为「✓ 已工作 + 发丝线」一行，点击逐层重开。
 * 相位由父层给（live / replied / done），相位 / 开关变化都丢弃手动覆盖、
 * 全映射落规格层（不搞边沿配对——running 瞬时翻假后层级必须能自愈，
 * v2.97 教训）。折叠是本地 UI 状态：不入 store、不持久化。「折叠工具执
 * 行过程」开关（v4.20 语义反转，缺省关）= 非执行块的规格层，保存即时
 * 生效（已挂载组随开关重落规格层）：关 = 展开（唯一被默认折叠的层是
 * 工具明细）、开 = 折叠为一行摘要。时长只在实时轮次有（历史
 * 重放无时间戳，不虚构时序）——AF-14 起读数只有一份（`TurnTimerReadout`），
 * 且**组头不再自带组内耗时**：容器第 0 层已经写着整轮的时长，组头再写一次
 * 就是同一属性两种形态（用户 2026-10-18 裁定）。
 */
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import { clip } from "../../lib/format";
import TurnTimerReadout from "./TurnTimerReadout.vue";

const props = withDefaults(
  defineProps<{
    /** v4.11 §3.39④：本轮被用户停止（收束后头行「已停止」，重放恒否）。 */
    stopped?: boolean;
    toolCount: number;
    failures: number;
    /** L1 极简摘要：块内动作名（去重保序，父层算好）。 */
    names?: string[];
    /** AF-14：容器计时读数的四个入参（与 `TurnTimerLine` 同形）。父层只在
     * **本块吞下了那条容器计时行**时才给（容器开着 + 本块是首个动作块 +
     * 非 live 相位）；undefined = 本块不显示读数。 */
    timer?: {
      startTs?: number;
      endTs?: number;
      activeMs?: number;
      running: boolean;
    };
    /** 块相位（父层按四层状态机计算）：live / replied / done。 */
    phase: "live" | "replied" | "done";
    defaultExpanded?: boolean;
  }>(),
  { phase: "done" },
);

const { t } = useI18n();

/** AF-14：组头行内的计时读数点了要折叠**整个轮容器**（第 0 层）——那是
 * `TranscriptFlow` 的事，本组件不自持容器开关。 */
const emit = defineEmits<{ toggleTurn: [] }>();

/** 规格层：执行中（live 相位，父层状态机唯一真源）恒开 = 组体常开、无组头
 * （读数也不再吃它——AF-14 起走表归 `TurnTimerReadout`）；非执行块按开关
 * （关 = 展开、开 = 折叠）。computed：开关保存即时生效——已挂载组也随开关
 * 重落规格层（v4.20 实机走查裁定：「不追溯」让开关看起来没生效）。 */
const specLevel = computed<1 | 2>(() =>
  props.phase === "live" ? 2 : props.defaultExpanded ? 2 : 1,
);

/** 当前层：1（组头）/ 2（组体展开）。L0 在 TurnTimerLine（v2.99 迁出）。
 * 手动开合覆盖优先；无覆盖时跟随规格层。 */
const manual = ref<1 | 2 | null>(null);
const level = computed<1 | 2>(() => manual.value ?? specLevel.value);

// 相位 / 开关变化 = 丢弃手动覆盖、全映射落规格层。不搞边沿配对——running
// 在轮界/重建时会瞬时翻假，配对式恢复会永远卡死（v2.97 白屏教训）；v4.20
// 起开关翻转同规：全局缺省变了，覆盖作废、重落规格层，层级自愈。
watch([() => props.phase, () => props.defaultExpanded], () => {
  manual.value = null;
});

// 收起即恢复裁剪（w121 修复）：is-settled 若带进收起态，0fr 盒高 0 但
// 内容溢出且 opacity 0 仍参与命中——透明点击盾压在后续行的组头上，
// 表现为「必须先展开上面的组才能点到下面的组」
watch(level, (v) => {
  if (v === 1) settled.value = false;
});

function onToggle() {
  manual.value = level.value === 2 ? 1 : 2;
}

/** 展开动画结束后才摘 overflow（否则聚焦环/引导线被裁）；收起动画结束
 * 一并归还（对称收口，兜住未经 onToggle 的收起路径）。 */
const settled = ref(false);
function onTransitionEnd(e: TransitionEvent) {
  if (e.propertyName !== "grid-template-rows") return;
  settled.value = level.value === 2;
}

// AF-14（用户 2026-10-18 裁定）：本组**不再自带组内耗时**。走表机器与读数文案
// 都搬进 `TurnTimerReadout`；组头原先这枚耗时（`34s`）与容器计时行（`0 分 34 秒`）
// 是同一属性两套写法、两个数字各算各的。容器那条读数现在并进本行走（`timer`
// 入参），本行只剩工具数。
const metaText = computed(
  () => `· ${t("turn.toolCount", { n: props.toolCount })}`,
);

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
         计时行）；点击开合组体。AF-14：行根由 `<button>` 改 `role="button"`
         的 div——行内要再嵌一颗「折叠轮容器」的按钮，按钮不能套按钮。 -->
    <div
      v-if="phase !== 'live'"
      class="cv-head"
      role="button"
      tabindex="0"
      :aria-expanded="level === 2"
      @click="onToggle"
      @keydown.enter.prevent="onToggle"
      @keydown.space.prevent="onToggle"
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
      <!-- AF-14：容器计时读数并进本行（父层只在把容器那条读数交给本块时才
           给 `timer`）。点它是折叠**整个轮容器**（第 0 层），与点行内别处
           （开合本组）是两件事 -->
      <template v-if="timer && timer.startTs !== undefined">
        <span class="cv-head__sep" aria-hidden="true">·</span>
        <button
          type="button"
          class="cv-head__timer"
          @click.stop="emit('toggleTurn')"
        >
          <TurnTimerReadout v-bind="timer" />
        </button>
      </template>
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
    </div>
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
/* 收起态组体不得参与命中（w121 防御层）：内容 opacity 0 仍会命中测试，
 * 一旦 overflow 因任何路径残留 visible，就是压在后续行上的透明点击盾
 * ——不可见的内容永远不该拦点击 */
.turn__grid:not(.is-open) .turn__body {
  pointer-events: none;
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
