<script setup lang="ts">
/**
 * 工具行（UI shell doc §3.8）——原「工具调用卡」降级为行：去掉卡片边框，
 * 用 wash 分层。折叠态 = 单行（字形 + 中文动作名 + 加工后参数摘要 + 状态
 * 点），禁止裸显工具 id（§3.18）；写/编辑类行尾带变更统计；hover 才出现
 * 复制按钮。展开态 = 明细块（bg-inset 圆角 10，参数/结果两段原样）。
 *
 * 默认一律收起（含运行中，用户裁定）；唯一例外 = 失败自动展开一次（实时
 * 到达与历史重放挂载同规，错误可见性优先）。defaultExpanded（设置「默认
 * 展开工具执行过程」）只对实时新发轮次生效，重放不吃。
 */
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Icon, type IconName } from "../../lib/icons";
import { toolAction, toolArgSummary, toolDiffStats } from "../../lib/tool-summary";
import type { ToolItem } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";

const props = defineProps<{
  item: ToolItem;
  /** 挂载时的默认展开（设置开关 × 实时轮次）；只在挂载时吃一次。 */
  defaultExpanded?: boolean;
}>();

const { t } = useI18n();
const settings = useSettingsStore();
const expanded = ref(props.defaultExpanded ?? false);

// 失败自动展开一次：实时结果到达（pending 翻转）与历史重放（挂载即失败）
// 同规；用户随后可手动折叠（手动切换后 watch 不再覆盖——failed 只翻转一次）。
watch(
  () => !props.item.pending && props.item.ok === false,
  (failed) => {
    if (failed) expanded.value = true;
  },
  { immediate: true },
);

/** 动作名（§3.18 映射表；表外回落工具 id + 「插件」徽章）。 */
const action = computed(() => toolAction(props.item.tool));
const actionLabel = computed(() => {
  const a = action.value;
  if (!a) return props.item.tool;
  return t(a.key, a.params ?? {});
});

/** 摘要态字形按工具类目分派（表外回落 check）。 */
const glyph = computed<IconName>(() => {
  const tool = props.item.tool;
  if (tool === "shell" || tool === "shell_read" || tool === "sandbox_run")
    return "terminal";
  if (tool.startsWith("fs_") || tool.startsWith("edit_")) return "file";
  if (tool.startsWith("memory_")) return "database";
  if (tool === "search_tools") return "search";
  if (tool === "subagent_run") return "bot";
  if (tool.startsWith("schedule_")) return "clock";
  if (tool === "notify") return "bell";
  if (tool === "fetch") return "globe";
  if (tool.startsWith("mcp_")) return "plug";
  return "check";
});

const argText = computed(() =>
  toolArgSummary(
    props.item.tool,
    props.item.args,
    [
      settings.view?.dataDir ?? "",
      ...(settings.view?.tools.fsRoots ?? []),
    ].filter(Boolean),
  ),
);

/** 写/编辑类折叠行尾的变更统计（取不到字段就不渲染整块，不虚构）。 */
const diff = computed(() =>
  props.item.pending ? {} : toolDiffStats(props.item.output),
);
const hasDiff = computed(
  () =>
    diff.value.added !== undefined ||
    diff.value.removed !== undefined ||
    diff.value.size !== undefined,
);

function pretty(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}

/** 复制调用：明细态原文（参数 + 结果 / 错误）。 */
async function copyCall() {
  const payload = {
    tool: props.item.tool,
    args: props.item.args,
    ...(props.item.ok ? { result: props.item.output } : {}),
    ...(props.item.error ? { error: props.item.error } : {}),
  };
  try {
    await navigator.clipboard.writeText(pretty(payload));
  } catch {
    /* 剪贴板不可用不打断 */
  }
}
</script>

<template>
  <div class="tool">
    <div class="tool__row">
      <button
        class="cv-head tool__toggle"
        type="button"
        :aria-expanded="expanded"
        :title="argText"
        @click="expanded = !expanded"
      >
        <Icon :name="glyph" :size="14" class="tool__icon shrink-0" />
        <span class="cv-head__label">{{ actionLabel }}</span>
        <span
          v-if="!action"
          class="shrink-0 rounded bg-wash-3 px-1.5 py-0.5 text-[10px] text-ink-2"
        >
          {{ t("tool.pluginBadge") }}
        </span>
        <span class="tool-args">{{ argText }}</span>
        <span
          v-if="hasDiff"
          class="tool__diff"
        >
          <template v-if="diff.added !== undefined || diff.removed !== undefined">
            <span class="tool__add">+{{ diff.added ?? 0 }}</span>
            <span class="tool__del">−{{ diff.removed ?? 0 }}</span>
          </template>
          <span v-if="diff.size" class="tool__size">{{ diff.size }}</span>
        </span>
        <span
          class="tool__status"
          :class="
            item.pending
              ? ''
              : item.ok
                ? 'tool__status--ok'
                : 'tool__status--fail'
          "
        >
          <span
            class="dot"
            :class="{
              'dot--ok': !item.pending && item.ok,
              'dot--danger': !item.pending && item.ok === false,
              'lw-pulse': item.pending,
            }"
          />
          {{
            item.pending
              ? t("tool.running")
              : item.ok
                ? t("tool.success")
                : t("tool.failed")
          }}
        </span>
        <Icon
          :name="expanded ? 'chevronDown' : 'chevronRight'"
          :size="12"
          class="cv-head__chev"
        />
      </button>
      <button
        class="copy-btn tool__copy"
        type="button"
        :title="t('tool.copyCall')"
        :aria-label="t('tool.copyCall')"
        @click="copyCall"
      >
        <Icon name="copy" :size="12" />
      </button>
    </div>
    <!-- 展开态 = 诚实面：参数原样 / 结果原样 / 失败 error 文本（danger） -->
    <div v-show="expanded" class="tool__detail">
      <div>
        <div class="tool__label">{{ t("tool.params") }}</div>
        <pre>{{ pretty(item.args) }}</pre>
      </div>
      <div v-if="!item.pending">
        <div class="tool__label">{{ t("tool.result") }}</div>
        <pre :class="{ 'is-error': item.ok === false }">{{
          item.ok ? pretty(item.output) : item.error
        }}</pre>
      </div>
    </div>
  </div>
</template>

<style scoped>
.tool__row {
  display: flex;
  align-items: center;
  gap: 2px;
}
.tool__toggle {
  /* basis 0 才能被兄弟（复制按钮）挤住；min-width:0 让参数摘要真正省略 */
  flex: 1 1 0%;
  width: auto;
  min-width: 0;
  margin-inline: -3px 0;
}
.tool__icon {
  color: var(--lw-ink-3);
}
.tool:hover .tool__icon {
  color: var(--lw-ink-2);
}
.tool-args {
  flex: 1 1 auto;
  min-width: 0;
  font-family: var(--font-mono);
  font-size: 12.5px;
  color: var(--lw-ink-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.tool__diff {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  flex: 0 1 auto;
  min-width: 0;
  overflow: hidden;
  white-space: nowrap;
  font-family: var(--font-mono);
  font-size: 11.5px;
}
.tool__add {
  color: var(--lw-ok);
}
.tool__del {
  color: var(--lw-danger);
}
.tool__size {
  color: var(--lw-ink-4);
}
.tool__status {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  flex: 0 0 auto;
  font-size: 12px;
  color: var(--lw-ink-3);
}
.tool__status--ok {
  color: var(--lw-ink-2);
}
.tool__status--fail {
  color: var(--lw-danger);
}
.tool__copy {
  opacity: 0;
  flex: 0 0 auto;
}
.tool:hover .tool__copy,
.tool:focus-within .tool__copy {
  opacity: 1;
}
.tool__detail {
  margin: 6px 0 8px;
  border-radius: var(--lw-r-sm);
  background: var(--lw-bg-inset);
  padding: 10px 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.tool__label {
  font-size: 10.5px;
  letter-spacing: 0.02em;
  color: var(--lw-ink-4);
  margin-bottom: 4px;
}
.tool__detail pre {
  margin: 0;
  font-family: var(--font-mono);
  font-size: 11.5px;
  line-height: 1.55;
  color: var(--lw-ink-2);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  max-height: 220px;
  overflow: auto;
}
.tool__detail pre.is-error {
  color: var(--lw-danger);
}
</style>
