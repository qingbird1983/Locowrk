<script setup lang="ts">
/**
 * 闲置徽章（壳批次骨架 + ⑩ 契约接线，exposure 文档 §8.4/§9.4/§11.2）：
 * 后端 plugin_list_with_usage 只给结构化 IdleInfo（level/value/unit），
 * 本组件做档位 → 颜色与"多久没用"人类文案的映射——后端不产文案，
 * 前端不解读裸数值。档位文案表见 locales idle.*（§8.6/§8.7）。
 *
 * 颜色映射（§9.4）：fresh/normal 不显示；idle 灰 / stale 黄 /
 * abandoned 红 / never_used 黄（"你可能不需要"≠"已坏"，黄档提示即可）。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import type { IdleInfo } from "../types";

const props = defineProps<{ idle: IdleInfo | null }>();
const { t } = useI18n();

/** 灰 / 黄 / 红三档（exposure 文档 §9.4）。 */
const tone = computed(() => {
  switch (props.idle?.level) {
    case "idle":
      return "bg-wash-3 text-ink-2"; // 中性
    case "stale":
    case "never_used":
      return "bg-warn/16 text-warn"; // 黄
    case "abandoned":
      return "bg-danger/16 text-danger"; // 红
    default:
      return ""; // fresh / normal：无标记
  }
});

const label = computed(() => {
  const i = props.idle;
  if (!i) return "";
  switch (i.unit) {
    case "today":
      return t("idle.today");
    case "this_week":
      return t("idle.this_week");
    case "last_week":
      return t("idle.last_week");
    case "last_month":
      return t("idle.last_month");
    case "month":
      return t("idle.month", { n: i.value });
    case "half_year":
      return t("idle.half_year");
    case "year":
      return t("idle.year", { n: i.value });
    case "never_used":
      // §8.7 从未使用：value 是安装月数档（None=两周内 / 1 / 3 / 6=半年多）
      if (i.value == null) return t("idle.never2w");
      if (i.value >= 6) return t("idle.neverHalfYear");
      return t("idle.neverMonths", { n: i.value });
    case "just_installed":
      return t("idle.just_installed");
    default:
      // unit 缺失（图例 / 降级显示）：用分级名
      return t(`idle.${i.level}`);
  }
});
</script>

<template>
  <span
    v-if="idle && tone"
    class="rounded-full px-2 py-0.5 text-[length:var(--lw-fs-10)] font-medium"
    :class="tone"
  >
    {{ label }}
  </span>
</template>
