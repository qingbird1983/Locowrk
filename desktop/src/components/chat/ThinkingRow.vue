<script setup lang="ts">
/**
 * 思考行（UI shell doc §3.9）：sparkle + 「思考」 + 正文单行截断（剥掉
 * markdown 记号与多余换行，只留纯文本首句）；点击展开为完整文本（12.5px
 * / 1.55 / ink-2，wash-1 底圆角 10）——展开后仍属"过程"，不抢正文视线。
 * 展开态原文照原样呈现（不篡改事实），摘要态才是人话。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../lib/icons";

const props = defineProps<{ text: string }>();
const { t } = useI18n();
const expanded = ref(false);

/** 摘要态：剥 markdown 记号（**、`）与换行，取首段有内容的句子。 */
const summary = computed(() => {
  const flat = props.text
    .replace(/[*`_#>]/g, "")
    .split(/\n+/)
    .map((s) => s.trim())
    .filter(Boolean);
  return flat[0] ?? "";
});
</script>

<template>
  <div class="thinking">
    <button
      class="cv-head thinking__header"
      type="button"
      :aria-expanded="expanded"
      @click="expanded = !expanded"
    >
      <Icon name="sparkle" :size="14" class="shrink-0 text-ink-3" />
      <span class="cv-head__label">{{ t("turn.thinking") }}</span>
      <span class="thinking__clip">{{ summary }}</span>
      <Icon
        :name="expanded ? 'chevronDown' : 'chevronRight'"
        :size="12"
        class="cv-head__chev"
      />
    </button>
    <!-- 展开体：v-show 保留 DOM（§3.8 折叠铁则），文本 pre-wrap 原样 -->
    <div v-show="expanded" class="thinking__body">{{ text }}</div>
  </div>
</template>

<style scoped>
.thinking__header {
  font-size: 12.5px;
}
.thinking__clip {
  flex: 1 1 auto;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--lw-ink-2);
}
.thinking__body {
  margin: 4px 0 6px;
  border-radius: var(--lw-r-sm);
  background: var(--lw-wash-1);
  padding: 10px 12px;
  font-size: 12.5px;
  line-height: 1.55;
  color: var(--lw-ink-2);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
</style>
