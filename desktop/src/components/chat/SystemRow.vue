<script setup lang="ts">
/**
 * 系统消息行（发丝线 / 卡片两档）+ 可选**展开体**。
 *
 * 为什么单独成文件：压缩线这一族要带展开体（摘要全文按模型正文那样渲染成
 * md），而转录里其它系统消息不带。展开体若写在 `TranscriptFlow.vue` 的模板
 * 里，那段 markup 会在三个落点各抄一份（轮容器内 / 顶层压缩线 / 尾段）⇒
 * 同一形态三份实现 = 迟早漂。本组件是这一族**唯一**的渲染落点（口径见
 * 《Front-end Design Guidelines》§二⑲「压缩线与摘要展开体」）。
 *
 * 形态：线仍是 `SystemNotice` 的两档（hairline 发丝线 / 居中窄卡片）；展开体
 * 是**模型正文形态**（`.md` + `renderMarkdown`）：整列宽、无底、无圆角、
 * 无边框、**永不折叠**（不受轮容器折叠管理）。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { renderMarkdown } from "../../lib/markdown";
import { useCopyFeedback } from "../../lib/use-copy";
import SystemNotice from "./SystemNotice.vue";

const props = defineProps<{
  /** 线上的那句话（i18n key 由调用方翻好传进来）。 */
  text: string;
  /** 发丝线形态（v2.143）：缺省 = 居中卡片窄条。 */
  hairline?: boolean;
  actions?: { id: string; label: string; tone?: "primary" | "ghost" }[];
  /** 展开体正文（**md 源**）：压缩线的摘要全文。缺省 = 这条没有展开体。 */
  body?: string;
  /** 展开态：true = 展开体在场（**这条线自己的状态**，不追加条目）。 */
  expanded?: boolean;
}>();

const emit = defineEmits<{ action: [id: string] }>();

const { t } = useI18n();

/** 摘要正文渲染成 md（与助手正文同一套解析 / 消毒 / 代码块增强）。 */
const html = computed(() =>
  props.body ? renderMarkdown(props.body, { copyCode: t("chat.copyCode") }) : "",
);

/** 代码块复制钮的委托（`v-html` 里的钮点不到 —— 与助手正文同一套做法）。 */
const { copyDelegatedCode } = useCopyFeedback();
</script>

<template>
  <div class="sys-row">
    <SystemNotice
      :text="text"
      :hairline="hairline"
      :actions="actions"
      @action="(id: string) => emit('action', id)"
    />
    <!-- 展开体：模型正文形态（整列宽 · 无底 · 无圆角 · 永不折叠） -->
    <!-- eslint-disable-next-line vue/no-v-html —— 内容已过 DOMPurify -->
    <div v-if="body && expanded" class="sys-body">
      <div class="md" v-html="html" @click="copyDelegatedCode" />
    </div>
  </div>
</template>

<style scoped>
/* 展开体（§二⑲）：**模型正文形态**——整列宽、无底、无圆角、无边框。
 * 纵向间距归自己（父级那条 8px 兄弟节奏管不到它：它在 `.sys-row` 内部）。 */
.sys-body {
  padding: var(--lw-gap-8) 0 0;
}
</style>
