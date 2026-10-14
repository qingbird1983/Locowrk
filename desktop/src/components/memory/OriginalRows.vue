<script setup lang="ts">
/**
 * 原文切片行（队列 Q14 第三刀 · §二⑯「汇聚面板」行的兑现件）。
 *
 * 为什么单开一个组件：§二⑯ 的原话是「面板内部行不许自造——用户 / 助手 / 工具三类行
 * 一律复用上位 §八 的既有形态」。这句话有两种做法：把 TreeSection 里那段行 markup
 * 抄进画布（= 造出第二套行，正是本节禁的东西），或把那套行提成**一个**组件、两个
 * 消费方共用。选后者——同类只许一种形态，形态的出处应该是文件而不是纪律条文。
 *
 * 字号档换了：原 TreeSection 用的是绝对档 `--lw-fs-11`/`--lw-fs-10`（存量迁移期
 * 别名），本节明确要画布内行落相对制 `--lw-fs-ui-*` ⇒ 共用件按新档走
 * （正文 `sm` = 基座−2、行首标记 `xs` = 基座−4）。这是「建了档」之后上位附录 C
 * #2 第二半的第一次实际迁移，其余存量仍按队列登记逐批替换。
 */
import { useI18n } from "vue-i18n";
import type { TreeOriginalItem } from "../../types";

const { t } = useI18n();

defineProps<{ rows: TreeOriginalItem[] }>();

/** 五种材料 kind（`MaterialKind` 的 serde 形，与后端 build_items 同源）。 */
function kindLabel(kind: TreeOriginalItem["kind"]): string {
  return t(
    kind === "user"
      ? "memory.kindUser"
      : kind === "assistant"
        ? "memory.kindAssistant"
        : kind === "tool_call"
          ? "memory.kindToolCall"
          : kind === "summary"
            ? "memory.kindSummary"
            : "memory.kindToolResult",
  );
}

/** 三类行的色档（§八 既有形态）：用户主字 / 助手次字 / 工具与摘要等宽最淡。 */
function rowClass(kind: TreeOriginalItem["kind"]): string {
  if (kind === "user") return "text-ink-1";
  if (kind === "assistant") return "text-ink-3";
  return "font-mono text-ink-5";
}

function isMono(kind: TreeOriginalItem["kind"]): boolean {
  return kind !== "user" && kind !== "assistant";
}
</script>

<template>
  <div class="space-y-1">
    <p
      v-for="row in rows"
      :key="row.index"
      class="break-words text-[length:var(--lw-fs-ui-sm)] leading-relaxed"
      :class="rowClass(row.kind)"
    >
      <span
        class="mr-1.5 select-none text-[length:var(--lw-fs-ui-xs)] text-ink-5"
        :class="isMono(row.kind) ? 'font-mono' : ''"
      >[{{ row.index }}] {{ kindLabel(row.kind) }}：</span
      >{{ row.text }}
    </p>
  </div>
</template>
