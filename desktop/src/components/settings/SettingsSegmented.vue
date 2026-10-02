<script setup lang="ts">
/**
 * 分段控件（UI shell 文档 §3.32·七 v3.6 两色控件体系；v3.7 滑动游标）：
 * 容器 = 白底台面（raised + shadow-raised）圆角 full + p-1；选中片 = 一片
 * 绝对定位的 --lw-bg-side 游标瓦片，按选中项 offsetLeft/Width 迁移（挂载/
 * 换值/换文案/字体就绪四时机校准；首测在显形前一帧完成，避免开场滑入），
 * 过渡走 .lw-seg-thumb（--dur-fast + --ease-out，style.css 全局层）。按钮
 * 本身只管文字色。v-model 绑 string；options[].label 由调用方 t() 后传入
 * （文案一律走 i18n，本组件不直接碰表）。全局同型控件（主题三档 / 用量
 * 读数二档）一律用它。
 */
import { nextTick, onMounted, ref, watch } from "vue";

const props = defineProps<{
  modelValue: string;
  options: ReadonlyArray<{ value: string; label: string }>;
  /** 无障碍名（radiogroup 的 aria-label，调用方 t() 后传入）。 */
  label: string;
}>();
defineEmits<{ "update:modelValue": [value: string] }>();

const btnRefs = ref<HTMLButtonElement[]>([]);
const thumb = ref({ left: 0, width: 0, ready: false });

/** 量测选中项几何 → 游标落点（offsetLeft 相对容器 padding 缘，与
 * 绝对定位参照系一致）。 */
function measure(): void {
  const i = props.options.findIndex((o) => o.value === props.modelValue);
  const el = btnRefs.value[i];
  if (!el) return;
  thumb.value = { ...thumb.value, left: el.offsetLeft, width: el.offsetWidth };
}

function setBtn(el: unknown, i: number): void {
  if (el instanceof HTMLButtonElement) btnRefs.value[i] = el;
}

watch(
  () => [props.modelValue, props.options] as const,
  () => void nextTick(measure),
);

onMounted(() => {
  void nextTick(() => {
    measure(); // 先定位（游标仍透明、无过渡），下一帧再显形
    requestAnimationFrame(() => {
      thumb.value = { ...thumb.value, ready: true };
    });
  });
  // 字体就绪后文字宽度会变（CJK/拉丁度量差），再量一次校准
  void document.fonts?.ready.then(() => measure());
});
</script>

<template>
  <div
    class="relative flex w-fit gap-1 rounded-full bg-raised p-1 shadow-raised"
    role="radiogroup"
    :aria-label="label"
  >
    <span
      aria-hidden="true"
      class="lw-seg-thumb absolute inset-y-1 rounded-full bg-side"
      :style="{
        left: `${thumb.left}px`,
        width: `${thumb.width}px`,
        opacity: thumb.ready ? 1 : 0,
      }"
    />
    <button
      v-for="(o, i) in options"
      :key="o.value"
      :ref="(el) => setBtn(el, i)"
      type="button"
      role="radio"
      :aria-checked="modelValue === o.value"
      class="relative z-10 rounded-full px-3 py-1 text-xs font-medium"
      :class="
        modelValue === o.value
          ? 'text-ink-1'
          : 'text-ink-4 hover:text-ink-2'
      "
      @click="$emit('update:modelValue', o.value)"
    >
      {{ o.label }}
    </button>
  </div>
</template>
