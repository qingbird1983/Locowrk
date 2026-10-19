<script setup lang="ts">
/**
 * 文件页签（specs/08 批 2；标准 §二⑱）：按路径读磁盘上的文本就地看——只读。
 * `.md` 走既有 `renderMarkdown`（marked + DOMPurify + shiki 代码块增强）；其余文本走
 * shiki 高亮 + 行号列（与 diff 卡同口径等宽）。数据 = `api.fileReadText`（上限 256 KiB，
 * 超了只回前缀 + 一行说明）；二进制 / 读不到都出一行说明，不出空白。
 * 头行几何在 style.css 的 `.lw-tabhead`（审阅 / 文件 / 图片三类产物页签共用）。
 */
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import HoverTip from "../settings/HoverTip.vue";
import { fileReadText } from "../../lib/api";
import { highlightCode } from "../../lib/highlight";
import { renderMarkdown } from "../../lib/markdown";
import { codeLanguage } from "../../lib/tool-detail/blocks";
import { fmtBytes } from "../../lib/tool-detail/labels";

const props = defineProps<{ path: string; line?: number; endLine?: number }>();
const { t } = useI18n();

const root = ref<HTMLElement | null>(null);
const loading = ref(true);
const error = ref("");
const bytes = ref(0);
const truncated = ref(false);
const binary = ref(false);
const text = ref("");

const fileName = computed(() => props.path.split(/[\\/]/).pop() ?? props.path);
const isMarkdown = computed(() => codeLanguage(props.path) === "markdown");
const bodyHtml = computed(() =>
  isMarkdown.value && text.value
    ? renderMarkdown(text.value, { copyCode: t("chat.copyCode") })
    : "",
);
/** 代码体按行切：shiki 的输出是一行一个 `<span class="line">`，整体高亮后再切——
 * 保住跨行上下文（块注释 / 模板串），逐行再包一层才拿得到行号列与目标行高亮。
 * 语言未收录 / 高亮器未就绪 ⇒ 回落纯文本行（不编造颜色）。 */
const codeLines = computed<{ html: boolean; parts: string[] }>(() => {
  if (isMarkdown.value || !text.value) return { html: false, parts: [] };
  const inner = highlightCode(text.value, codeLanguage(props.path));
  return inner === null
    ? { html: false, parts: text.value.split("\n") }
    : { html: true, parts: inner.split("\n") };
});
/** 目标行区间（1 起、含两端）：来自点击入口（代码块头 / diff 行号），滚动 + 高亮。 */
const firstLine = computed(() => props.line ?? 0);
const lastLine = computed(() => Math.max(firstLine.value, props.endLine ?? firstLine.value));
function isTarget(no: number): boolean {
  return firstLine.value > 0 && no >= firstLine.value && no <= lastLine.value;
}

async function load() {
  loading.value = true;
  error.value = "";
  text.value = "";
  try {
    const dto = await fileReadText(props.path);
    bytes.value = dto.bytes;
    truncated.value = dto.truncated;
    binary.value = dto.binary;
    text.value = dto.text;
    await nextTick();
    root.value?.querySelector<HTMLElement>("[data-target]")?.scrollIntoView({
      block: "center",
    });
  } catch (e) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

watch(() => [props.path, props.line, props.endLine], () => void load(), {
  immediate: true,
});
</script>

<template>
  <div ref="root" class="scroll-y min-h-0 flex-1 px-3 py-2">
    <div class="lw-tabhead">
      <HoverTip class="min-w-0" :text="path" :label="fileName">
        <span class="lw-tabhead__path">{{ fileName }}</span>
      </HoverTip>
      <span class="lw-tabhead__meta">
        <span v-if="!loading && !error">{{ fmtBytes(bytes) }}</span>
      </span>
    </div>

    <p v-if="loading" class="ftab__note">{{ t("panel.fileLoading") }}</p>
    <p v-else-if="error" class="ftab__note">{{ error }}</p>
    <p v-else-if="binary" class="ftab__note">{{ t("panel.notText") }}</p>
    <template v-else>
      <div v-if="isMarkdown" class="md mt-2" v-html="bodyHtml" />
      <div v-else class="ftab__code mt-2">
        <div
          v-for="(part, i) in codeLines.parts"
          :key="i"
          class="ftab__line"
          :class="{ 'ftab__line--target': isTarget(i + 1) }"
          :data-target="isTarget(i + 1) ? '' : undefined"
        >
          <span class="ftab__no" aria-hidden="true">{{ i + 1 }}</span>
          <span v-if="codeLines.html" class="ftab__src" v-html="part" />
          <span v-else class="ftab__src">{{ part }}</span>
        </div>
      </div>
      <p v-if="truncated" class="ftab__note">
        {{ t("panel.fileTruncated", { n: fmtBytes(bytes) }) }}
      </p>
    </template>
  </div>
</template>

<style scoped>
.ftab__note {
  padding: var(--lw-gap-12) var(--lw-gap-4);
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-4);
}
/* 代码体（标准 §二⑱）：井口径底 + 行号列 32px 右对齐 mono（与 diff 卡同宽） */
.ftab__code {
  border-radius: var(--lw-r-md);
  background: var(--lw-bg-inset);
  padding: var(--lw-gap-8) 0;
  overflow-x: auto;
}
.ftab__line {
  display: flex;
  align-items: baseline;
  min-height: 20px;
  font-family: var(--font-mono);
  font-size: var(--lw-fs-12);
  line-height: 20px;
  color: var(--lw-ink-2);
}
.ftab__line--target {
  background: var(--lw-wash-2);
}
.ftab__no {
  width: 32px;
  flex: 0 0 auto;
  padding-right: var(--lw-gap-4);
  text-align: right;
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-4);
  user-select: none;
}
.ftab__src {
  min-width: 0;
  flex: 1 1 auto;
  padding-right: var(--lw-gap-8);
  white-space: pre;
}
</style>
