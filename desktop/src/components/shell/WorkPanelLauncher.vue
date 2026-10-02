<script setup lang="ts">
/**
 * 工作面板启动器（§8.4 WorkPanelLauncher，自 WorkPanel 拆出）：面板空态 =
 * 启动器清单而非空态文案（§3.14）。只列已实装页签类型的条目：§3.16 当前
 * 会话收束的 subagent_run（子转录）+ §3.28 当前会话的 edit_diff（审阅，
 * diff 随 args.patch 快照）。行形态照原型 .launcher__row（wash-1 卡、
 * 图标 + 标题 + 行尾 hint）。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  useChatStore,
  type ToolItem,
} from "../../stores/chat";
import { clip } from "../../lib/format";
import { Icon } from "../../lib/icons";

const emit = defineEmits<{
  openSub: [childId: string, title: string];
  openReview: [item: { callId: string; path: string; diff: string }];
}>();

const { t } = useI18n();
const chat = useChatStore();

/** §3.16 子转录：当前会话收束的 subagent_run。 */
const subItems = computed(() => {
  const out: { childId: string; title: string }[] = [];
  const seen = new Set<string>();
  for (const item of chat.items) {
    if (item.kind !== "tool" || item.pending || !item.ok) continue;
    const tool = item as ToolItem;
    if (tool.tool !== "subagent_run") continue;
    const childId = (tool.output as { child_session_id?: string } | null)?.child_session_id;
    if (!childId || seen.has(childId)) continue;
    seen.add(childId);
    out.push({
      childId,
      title: clip(String((tool.args as { prompt?: unknown } | null)?.prompt ?? ""), 40),
    });
  }
  return out;
});

/** §3.28 审阅：当前会话的 edit_diff 调用（diff 随 args.patch）。 */
const reviewItems = computed(() => {
  const out: { callId: string; path: string; diff: string }[] = [];
  const seen = new Set<string>();
  for (const item of chat.items) {
    if (item.kind !== "tool" || item.pending) continue;
    const tool = item as ToolItem;
    if (tool.tool !== "edit_diff") continue;
    const args = tool.args as { path?: unknown; patch?: unknown } | null;
    if (typeof args?.path !== "string" || typeof args.patch !== "string") continue;
    if (seen.has(tool.callId)) continue;
    seen.add(tool.callId);
    out.push({ callId: tool.callId, path: args.path, diff: args.patch });
  }
  return out;
});
</script>

<template>
  <div class="launcher scroll-y flex-1 px-3 pb-4 pt-2.5">
    <template v-if="subItems.length > 0 || reviewItems.length > 0">
      <template v-if="subItems.length > 0">
        <div class="side-label px-1">{{ t("panel.launcherSub") }}</div>
        <button
          v-for="item in subItems"
          :key="item.childId"
          type="button"
          class="launcher-row"
          @click="emit('openSub', item.childId, item.title)"
        >
          <Icon name="bot" :size="14" aria-hidden="true" />
          <span class="launcher-row__title">{{ item.title }}</span>
          <span class="launcher-row__hint">{{
            t("panel.launcherSubHint")
          }}</span>
        </button>
      </template>
      <template v-if="reviewItems.length > 0">
        <div class="side-label px-1">{{ t("panel.launcherReview") }}</div>
        <button
          v-for="item in reviewItems"
          :key="item.callId"
          type="button"
          class="launcher-row"
          :title="item.path"
          @click="emit('openReview', item)"
        >
          <Icon name="file" :size="14" aria-hidden="true" />
          <span class="launcher-row__title">{{
            item.path.split(/[\\/]/).pop() ?? item.path
          }}</span>
          <span class="launcher-row__hint">{{
            t("panel.launcherReviewHint")
          }}</span>
        </button>
      </template>
    </template>
    <p class="px-2 py-3 text-[12px] leading-relaxed text-ink-4">
      {{ t("panel.launcherHint") }}
    </p>
  </div>
</template>

<style scoped>
/* 启动器（原型 .launcher__row）：wash-1 卡、圆角 10、10px 12px、
 * 图标 + 标题 + 行尾 hint（11.5px ink-4），行间 6px */
.launcher {
  display: flex;
  flex-direction: column;
  gap: 6px;
}
.launcher-row {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  border-radius: var(--lw-r-sm);
  background: var(--lw-wash-1);
  padding: 10px 12px;
  font-size: 13px;
  color: var(--lw-ink-1);
  text-align: left;
}
.launcher-row:hover {
  background: var(--lw-wash-2);
}
.launcher-row > svg {
  color: var(--lw-ink-3);
}
.launcher-row__title {
  min-width: 0;
  flex: 0 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.launcher-row__hint {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-left: auto;
  text-align: right;
  font-size: 11.5px;
  color: var(--lw-ink-4);
}
.side-label {
  padding: 6px 8px 4px;
  font-size: 11.5px;
  color: var(--lw-ink-4);
}
</style>
