<script setup lang="ts">
/**
 * 分类徽章（S1 刀④c · ADR-0007 §18 里管理页那一列）：与 `SourceBadge` **同一形态**
 * （§3.4 徽章表同尺寸那颗 `rounded-full px-2 py-0.5`），只换配色映射——同类只许一种形态，
 * 所以这里不裸写 `<span>`，也不给分类另造一颗新形状。
 *
 * 值域由**后端**收档：`memlayout::normalize_type` 把四类型之外的一切（旧文件的 `fact`、
 * 人乱写的、根本没这行）都读成 `UNCLASSIFIED_TYPE` ⇒ 这一层只认六个键。未知串一样走
 * 未分类那一档的文案，不原样打印：把 `reference ` 这种串直接甩在界面上，用户读到的是
 * 机器词而不是"这条没分类"那句人话；留空位更糟——会被读成"这一列坏了"。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { memoryTypeLabelKey } from "../lib/memory-type";

const props = defineProps<{
  /** "user" | "feedback" | "project" | "reference" | "fact"（= 未分类档）。 */
  type: string;
}>();

const { t } = useI18n();
const label = computed(() => t(memoryTypeLabelKey(props.type)));

/**
 * 配色只用**这一族已经在用的**四档（info / warn / purple / wash-3）加一档更中性的 wash-2：
 * 分类是次要维度，未分类那一档要比四类型更不起眼（它多数时候意味着"这是重构前记的老条"），
 * 而不是去做第五第六个色相。
 */
const tone = computed(() => {
  switch (props.type) {
    case "user":
      return "bg-info/16 text-info";
    case "feedback":
      return "bg-warn/16 text-warn";
    case "project":
      return "bg-purple/16 text-purple";
    case "reference":
      return "bg-wash-3 text-ink-2";
    default:
      return "bg-wash-2 text-ink-4";
  }
});
</script>

<template>
  <span
    class="inline-flex shrink-0 items-center rounded-full px-2 py-0.5 text-[length:var(--lw-fs-10)] leading-none"
    :class="tone"
  >
    {{ label }}
  </span>
</template>
