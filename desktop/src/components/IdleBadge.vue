<script setup lang="ts">
/**
 * 闲置徽章（壳批次，docs/design/UI shell and patterns.md §3.4）：
 * ⑩ 起 plugin_list_with_usage 提供 IdleInfo，本组件只做结构化数据 →
 * 颜色与"多久没用"人类文案的映射（exposure 文档 §8.4/§9.4）——后端
 * 不产文案，前端不解读裸数值。
 *
 * 颜色映射：fresh/normal 不显示；idle 灰 / stale 黄 / abandoned 与
 * never_used 红。有 value+unit 时徽章文案取时长（如"14 天未使用"），
 * 否则退化为分级名（筛选器/图例用）。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";

/** 闲置信息（后端 IdleInfo 契约，⑩ 落地；level 取六级分类 key）。 */
export interface IdleInfo {
  level: "fresh" | "normal" | "idle" | "stale" | "abandoned" | "never_used";
  value?: number | null;
  /** day / week / hour / minute（i18n idle.unit.* 渲染）。 */
  unit?: string | null;
}

const props = defineProps<{ idle: IdleInfo | null }>();
const { t } = useI18n();

/** 灰 / 黄 / 红 三档（exposure 文档 §9.4）。 */
const tone = computed(() => {
  switch (props.idle?.level) {
    case "idle":
      return "bg-neutral-700/60 text-neutral-300"; // 灰
    case "stale":
      return "bg-yellow-900/40 text-yellow-300"; // 黄
    case "abandoned":
    case "never_used":
      return "bg-red-900/40 text-red-300"; // 红
    default:
      return "";
  }
});

const label = computed(() => {
  const i = props.idle;
  if (!i) return "";
  if (i.value != null && i.unit) {
    return t("idle.unusedFor", { n: i.value, unit: t(`idle.unit.${i.unit}`) });
  }
  return t(`idle.${i.level}`);
});
</script>

<template>
  <span
    v-if="idle && tone"
    class="rounded-full px-2 py-0.5 text-[10px] font-medium"
    :class="tone"
  >
    {{ label }}
  </span>
</template>
