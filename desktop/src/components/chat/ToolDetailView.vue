<script setup lang="ts">
/**
 * 工具明细呈现（UI shell doc §3.8 v4.13）：吃 lib/tool-detail.ts 的视图
 * 模型，按块渲染——props（标签+值两列属性行）/ code（.md 代码块组件复用：
 * 语言标签头行 + 复制钮，走 renderMarkdown 围栏注入）/ text（mono 输出块，
 * 220px 框内滚动）/ list（条目列表）/ entry（子代理「查看完整对话」文本钮）。
 * 失败态 = error 文本 danger 块。底部「原始数据 ▸」默认收起（给工程师）——
 * 展开才是 pretty JSON。默认视图零裸 JSON（自检红线 §3.8）。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";
import { renderMarkdown } from "../../lib/markdown";
import { toolDetail } from "../../lib/tool-detail";
import { useCopyFeedback } from "../../lib/use-copy";
import { useSettingsStore } from "../../stores/settings";
import type { ToolItem } from "../../stores/chat";

const props = defineProps<{ item: ToolItem }>();
const emit = defineEmits<{ openTranscript: [childId: string, title: string] }>();

const { t } = useI18n();
const settings = useSettingsStore();
const showRaw = ref(false);

const ctx = computed(() => ({
  roots: [settings.view?.dataDir ?? "", ...(settings.view?.tools.fsRoots ?? [])].filter(Boolean),
  tr: (key: string, params?: Record<string, unknown>) => t(key, params ?? {}),
}));

const detail = computed(() =>
  toolDetail(props.item.tool, props.item.args, props.item.ok ? props.item.output : null, ctx.value),
);

/** 失败轮：错误文本是第一块（danger），输出缺席不虚构。 */
const errorText = computed(() =>
  props.item.ok === false ? (props.item.error ?? t("tool.failed")) : "",
);

/** 代码块经 renderMarkdown（DOMPurify 消毒 + 代码块增强）。围栏按内容里
 * 最长的反引号串自适应加长，防止正文中的 ``` 截断围栏。 */
const codeHtml = computed(() => {
  const blocks = detail.value.blocks.filter((b) => b.kind === "code");
  if (blocks.length === 0) return "";
  const parts = blocks.map((b) => {
    if (b.kind !== "code") return "";
    const runs = b.content.match(/`{3,}/g);
    const longest = runs ? Math.max(...runs.map((r) => r.length)) : 0;
    const fence = "`".repeat(Math.max(3, longest + 1));
    return `${fence}${b.language}\n${b.content}\n${fence}`;
  });
  return renderMarkdown(parts.join("\n\n"), { copyCode: t("tool.copyCode") });
});

// 复制反馈（§3.7 v4.18）：代码块委托钮走 DOM class、复制调用钮走 ref
const { copiedId, copyWithFeedback, flashDom } = useCopyFeedback();

async function onCodeClick(e: MouseEvent) {
  const target = (e.target as HTMLElement).closest?.(".copy-btn");
  if (!target) return;
  const fig = target.closest(".code-block");
  const code = fig?.querySelector("pre code")?.textContent ?? "";
  try {
    await navigator.clipboard.writeText(code);
    flashDom(target);
  } catch {
    /* 剪贴板不可用不打断 */
  }
}

const rawJson = computed(() => {
  const payload = {
    tool: props.item.tool,
    args: props.item.args,
    ...(props.item.ok ? { result: props.item.output } : {}),
    ...(props.item.error ? { error: props.item.error } : {}),
  };
  try {
    return JSON.stringify(payload, null, 2);
  } catch {
    return String(payload);
  }
});

async function copyCall() {
  await copyWithFeedback(rawJson.value, "call");
}
</script>

<template>
  <div class="tdetail">
    <!-- 失败：错误文本（danger 平铺，无卡片底） -->
    <p v-if="errorText" class="tdetail__error">{{ errorText }}</p>

    <!-- 属性行 / 列表 / 文本块 -->
    <template v-for="(b, i) in detail.blocks" :key="i">
      <dl v-if="b.kind === 'props'" class="tdetail__props">
        <div v-for="r in b.rows" :key="r.label + r.value" class="tdetail__prop">
          <dt>{{ r.label }}</dt>
          <dd :class="{ 'is-mono': r.tone === 'mono', 'is-ok': r.tone === 'ok', 'is-danger': r.tone === 'danger' }">
            {{ r.value }}
          </dd>
        </div>
      </dl>

      <ul v-else-if="b.kind === 'list'" class="tdetail__list">
        <li v-for="r in b.items" :key="r.title + (r.meta ?? '')">
          <Icon :name="r.icon === 'folder' ? 'folder' : r.icon === 'link' ? 'globe' : r.icon === 'tool' ? 'plug' : 'file'" :size="12" class="tdetail__list-icon" />
          <span class="tdetail__list-title">{{ r.title }}</span>
          <span v-if="r.meta" class="tdetail__list-meta">{{ r.meta }}</span>
        </li>
      </ul>

      <pre v-else-if="b.kind === 'text'" class="tdetail__text" :class="{ 'is-danger': b.danger }">{{ b.content }}</pre>

      <button
        v-else-if="b.kind === 'entry'"
        class="tdetail__entry"
        type="button"
        @click="emit('openTranscript', b.childId, b.title)"
      >
        <Icon name="panel" :size="12" />
        {{ t("tool.fullConversation") }}
      </button>
    </template>

    <!-- 代码块（fs_read 内容等）：.md 全局代码块形态复用 -->
    <!-- eslint-disable-next-line vue/no-v-html —— 内容已过 DOMPurify -->
    <div
      v-if="codeHtml"
      class="md tdetail__code"
      v-html="codeHtml"
      @click="onCodeClick"
    />

    <!-- 原始数据入口 + 复制调用（文本钮，默认收起） -->
    <div class="tdetail__foot">
      <button class="tdetail__entry" type="button" :aria-expanded="showRaw" @click="showRaw = !showRaw">
        <Icon :name="showRaw ? 'chevronDown' : 'chevronRight'" :size="12" />
        {{ t("tool.rawData") }}
      </button>
      <button
        class="tdetail__entry"
        :class="{ 'is-copied': copiedId === 'call' }"
        type="button"
        @click="copyCall"
      >
        <Icon :name="copiedId === 'call' ? 'check' : 'copy'" :size="12" />
        {{ t("tool.copyCall") }}
      </button>
    </div>
    <pre v-show="showRaw" class="tdetail__text tdetail__raw">{{ rawJson }}</pre>
  </div>
</template>

<style scoped>
.tdetail {
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-8);
  min-width: 0;
}
.tdetail__error {
  margin: 0;
  font-size: var(--lw-fs-12);
  line-height: 1.5;
  color: var(--lw-danger);
  overflow-wrap: anywhere;
}
/* 标签 + 值两列属性行（§3.8/§五）：无卡片框，标签列定宽对齐 */
.tdetail__props {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 3px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值3,§二⑬) */
}
.tdetail__prop {
  display: grid;
  grid-template-columns: 72px minmax(0, 1fr);
  column-gap: var(--lw-gap-8);
  align-items: baseline;
}
.tdetail__prop dt {
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-2);
}
.tdetail__prop dd {
  margin: 0;
  min-width: 0;
  font-size: var(--lw-fs-12);
  line-height: 1.5;
  color: var(--lw-ink-2);
  overflow-wrap: anywhere;
}
.tdetail__prop dd.is-mono {
  font-family: var(--font-mono);
  font-size: var(--lw-fs-12);
}
.tdetail__prop dd.is-ok {
  color: var(--lw-ok);
}
.tdetail__prop dd.is-danger {
  color: var(--lw-danger);
}
/* 条目列表（目录/匹配/键值命中） */
.tdetail__list {
  margin: 0;
  padding: 0;
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 2px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值2,§二⑬) */
}
.tdetail__list li {
  display: flex;
  align-items: baseline;
  gap: var(--lw-gap-6);
  min-width: 0;
  font-size: var(--lw-fs-12);
  line-height: 1.5;
}
.tdetail__list-icon {
  flex: 0 0 auto;
  align-self: center;
  color: var(--lw-ink-2);
}
.tdetail__list-title {
  flex: 0 0 auto;
  color: var(--lw-ink-2);
  font-family: var(--font-mono);
}
.tdetail__list-meta {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--lw-ink-2);
  font-family: var(--font-mono);
  font-size: var(--lw-fs-11);
}
/* mono 输出块：框内滚动（v2.97 裁定 220px 同族），无卡片底 */
.tdetail__text {
  margin: 0;
  max-height: 220px;
  overflow: auto;
  font-family: var(--font-mono);
  font-size: var(--lw-fs-12);
  line-height: 1.55;
  color: var(--lw-ink-2);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.tdetail__text.is-danger {
  color: var(--lw-danger);
}
/* 代码内容 = .md 代码块组件复用；长内容同样框内滚动 */
.tdetail__code {
  font-size: var(--lw-fs-12);
}
.tdetail__code :deep(.code-block) {
  margin: 0;
  max-height: 260px;
  overflow-y: auto;
}
/* 动作行：文本钮（查看完整对话 / 原始数据 / 复制调用）——非卡片 */
.tdetail__foot {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
}
.tdetail__entry {
  display: inline-flex;
  align-items: center;
  gap: 4px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值4,§二⑬) */
  padding: 2px 4px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值2/4,§二⑬) */
  border-radius: var(--lw-r-2xs);
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-2);
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.tdetail__entry:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
/* 复制反馈（§3.7 v4.18）：复制调用成功 = 字形换对勾（icons name 已换）
 * + ok 绿驻留 ≈1.6s；排在 :hover 后靠源序取胜（同特异性） */
.tdetail__entry.is-copied {
  color: var(--lw-ok);
}
.tdetail__raw {
  color: var(--lw-ink-2);
  font-size: var(--lw-fs-11);
}
</style>
