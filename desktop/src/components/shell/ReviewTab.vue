<script setup lang="ts">
/**
 * 审阅页签（UI shell 文档 §3.28，v2.53）：工作面板内**只读**的 unified
 * diff 渲染——文件名头（左路径 mono 省略 / 右 +N −M 统计）+ 整行染底
 * （ok/danger 8%）+ 行首 1px 实色条 + 行号 32px 右对齐 + 折叠上下文
 * （未变更 >3 行折叠、`… 展开 N 行` 展开一次性不收回）。数据 = 页签快照
 * （edit_diff 的 args.patch），本页签不提供编辑。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import HoverTip from "../settings/HoverTip.vue";
import { collapseContext, parseUnifiedDiff } from "../../lib/diff-parse";

const props = defineProps<{ path: string; diff: string }>();
const { t } = useI18n();

const parsed = computed(() => parseUnifiedDiff(props.diff));
/** 展开过的折叠块（`${hunk 序}-${块序}`，只增不收回——审阅以看完为目的）。 */
const expanded = ref(new Set<string>());

function toggleExpand(key: string) {
  expanded.value = new Set([...expanded.value, key]);
}

const fileName = computed(() => props.path.split(/[\\/]/).pop() ?? props.path);
</script>

<template>
  <div class="scroll-y min-h-0 flex-1 px-3 py-2">
    <div
      v-if="parsed.hunks.length === 0"
      class="px-1 py-4 text-center text-[12px] text-ink-4"
    >
      {{ t("panel.diffEmpty") }}
    </div>
    <template v-else>
      <!-- 文件名头（§3.28）：wash-1 底，左路径 mono / 右统计 -->
      <div class="diff-head">
        <HoverTip class="min-w-0" :text="props.path" :label="props.path">
          <span class="diff-head__path">{{ fileName }}</span>
        </HoverTip>
        <span class="diff-head__stats">
          <span class="text-ok">+{{ parsed.added }}</span>
          <span class="text-danger">−{{ parsed.removed }}</span>
        </span>
      </div>

      <div
        v-for="(hunk, hi) in parsed.hunks"
        :key="hi"
        class="mb-3"
      >
        <template v-for="(block, bi) in collapseContext(hunk.lines)" :key="bi">
          <!-- 折叠中缝：… 展开 N 行（只增不收回） -->
          <button
            v-if="block.kind === 'collapsed' && !expanded.has(`${hi}-${bi}`)"
            type="button"
            class="diff-expand"
            @click="toggleExpand(`${hi}-${bi}`)"
          >
            {{ t("panel.expandLines", { n: block.count }) }}
          </button>
          <template v-if="block.kind === 'lines' || expanded.has(`${hi}-${bi}`)">
            <div
              v-for="(l, li) in block.lines"
              :key="`${hi}-${bi}-${li}`"
              class="diff-line"
              :class="`diff-line--${l.type}`"
            >
              <span class="diff-line__no">{{ l.oldNo ?? "" }}</span>
              <span class="diff-line__no">{{ l.newNo ?? "" }}</span>
              <span class="diff-line__text">{{ l.text }}</span>
            </div>
          </template>
        </template>
      </div>
    </template>
  </div>
</template>

<style scoped>
.diff-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  border-radius: var(--lw-r-md);
  background: var(--lw-wash-1);
  padding: 12px 14px;
  font-size: 12px;
}
.diff-head__path {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--font-mono);
  color: var(--lw-ink-2);
}
.diff-head__stats {
  flex: 0 0 auto;
  display: inline-flex;
  gap: 6px;
  font-family: var(--font-mono);
  font-size: 11px;
}

/* 行（§3.28）：整行染底 8% + 行首 1px 实色条 + 行号 32px 右对齐 mono */
.diff-line {
  display: flex;
  align-items: baseline;
  min-height: 20px;
  font-family: var(--font-mono);
  font-size: 11.5px;
  line-height: 20px;
  color: var(--lw-ink-2);
}
.diff-line--add {
  background: color-mix(in oklab, var(--lw-ok) 8%, transparent);
  box-shadow: inset 1px 0 0 var(--lw-ok);
}
.diff-line--del {
  background: color-mix(in oklab, var(--lw-danger) 8%, transparent);
  box-shadow: inset 1px 0 0 var(--lw-danger);
}
.diff-line__no {
  width: 32px;
  flex: 0 0 auto;
  padding-right: 4px;
  text-align: right;
  font-size: 10.5px;
  color: var(--lw-ink-4);
  user-select: none;
}
.diff-line__text {
  min-width: 0;
  flex: 1 1 auto;
  white-space: pre-wrap;
  word-break: break-all;
  padding-right: 8px;
}

.diff-expand {
  display: block;
  width: 100%;
  padding: 2px 0;
  text-align: center;
  font-size: 11px;
  color: var(--lw-ink-4);
  background: var(--lw-wash-1);
}
.diff-expand:hover {
  color: var(--lw-ink-1);
  background: var(--lw-wash-2);
}
</style>
