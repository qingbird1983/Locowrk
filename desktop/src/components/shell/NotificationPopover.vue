<script setup lang="ts">
/**
 * 通知浮层（UI shell doc §3.13，v2.51）：teleport body + fixed 锚在侧栏底
 * 通知图标上方（380px，可溢出侧栏右缘——§五 浮层家族、挂根纪律 §2.6；
 * 侧栏 overflow-hidden 装不下它）。头 = 通知 + 全部已读 ✓ + 清空 🗑；
 * 分段 全部/未读；行 = 未读 6px accent 点 + 类别标题（i18n 模板，记录只
 * 存 kind+参数）+ 正文（数据参数：调度名/会话标题/省下 token/错误摘要）+
 * 相对时间（§3.5 共享实现）。行点击 = 单条置读 + 有会话归属则跳转。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { useNotificationsStore } from "../../stores/notifications";
import { useShellStore } from "../../stores/shell";
import { compactNumber, relativeTime } from "../../lib/format";
import { Icon } from "../../lib/icons";
import HoverTip from "../settings/HoverTip.vue";
import type { NotificationItem } from "../../types";

/** 触发钮的视口坐标（左上角）。 */
const props = defineProps<{ x: number; y: number }>();
const emit = defineEmits<{ close: [] }>();
const { t } = useI18n();
const store = useNotificationsStore();
const chat = useChatStore();
const shell = useShellStore();

const filter = ref<"all" | "unread">("all");
const filtered = computed(() =>
  filter.value === "all" ? store.items : store.items.filter((i) => !i.read),
);

const root = ref<HTMLElement | null>(null);
const pos = ref({ left: 0, bottom: 0 });

function onOutside(e: PointerEvent) {
  if (!root.value?.contains(e.target as Node)) emit("close");
}

onMounted(() => {
  pos.value = {
    left: Math.max(8, Math.min(props.x, window.innerWidth - 380 - 8)),
    bottom: window.innerHeight - props.y + 8,
  };
  document.addEventListener("pointerdown", onOutside, true);
});
onBeforeUnmount(() =>
  document.removeEventListener("pointerdown", onOutside, true),
);

function kindTitle(item: NotificationItem): string {
  switch (item.kind) {
    case "schedule_fired":
      return t("notify.scheduleFired");
    case "subagent_done":
      return t("notify.subagentDone");
    case "subagent_failed":
      return t("notify.subagentFailed");
    case "context_compacted":
      return t("notify.contextCompacted");
    case "approval_timeout":
      return t("notify.approvalTimeout");
  }
}

function sessionTitle(item: NotificationItem): string | undefined {
  return item.sessionId
    ? chat.sessions.find((s) => s.id === item.sessionId)?.title ?? undefined
    : undefined;
}

function bodyText(item: NotificationItem): string {
  const ctx = sessionTitle(item) ?? item.sessionId ?? "";
  switch (item.kind) {
    case "schedule_fired":
      return [item.label, ctx].filter(Boolean).join(" · ") || t("notify.noContext");
    case "subagent_failed":
      return item.label ?? (ctx || t("notify.noContext"));
    case "subagent_done":
    case "approval_timeout":
      return ctx || t("notify.noContext");
    case "context_compacted":
      return t("notify.savedTokens", {
        n: item.label ? compactNumber(Number(item.label)) : "—",
      });
  }
}

function timeText(item: NotificationItem): string {
  const r = relativeTime(item.createdAt);
  return r.n !== undefined
    ? t(`memory.time.${r.key}`, { n: r.n })
    : t(`memory.time.${r.key}`);
}

async function onRow(item: NotificationItem) {
  if (!item.read) await store.markRead(item.id);
  if (item.sessionId && item.sessionId !== chat.sessionId && !chat.running) {
    void chat.switchSession(item.sessionId);
    shell.setDestination("chat");
  }
  emit("close");
}
</script>

<template>
  <!-- 不自带 Teleport：挂载点用 <Teleport><Transition name="lw-pop-up">
       包裹（进退场由此生效，标准 §二① 禁止无退场） -->
  <div
    ref="root"
    class="notif-pop"
    role="dialog"
    :aria-label="t('notify.title')"
    :style="{ left: `${pos.left}px`, bottom: `${pos.bottom}px` }"
  >
      <div class="notif-pop__head">
        <span class="notif-pop__title">{{ t("notify.title") }}</span>
        <HoverTip :text="t('notify.markAll')">
          <button
            class="notif-pop__act"
            type="button"
            :aria-label="t('notify.markAll')"
            :disabled="store.unread === 0"
            @click="store.markAllRead()"
          >
            <Icon name="check" :size="14" aria-hidden="true" />
          </button>
        </HoverTip>
        <HoverTip :text="t('notify.clear')">
          <button
            class="notif-pop__act"
            type="button"
            :aria-label="t('notify.clear')"
            :disabled="store.items.length === 0"
            @click="store.clear()"
          >
            <Icon name="trash" :size="14" aria-hidden="true" />
          </button>
        </HoverTip>
      </div>

      <div class="notif-pop__seg" role="tablist">
        <button
          type="button"
          role="tab"
          :aria-selected="filter === 'all'"
          :data-on="filter === 'all' || undefined"
          @click="filter = 'all'"
        >
          {{ t("notify.all") }}
        </button>
        <button
          type="button"
          role="tab"
          :aria-selected="filter === 'unread'"
          :data-on="filter === 'unread' || undefined"
          @click="filter = 'unread'"
        >
          {{ t("notify.unread") }}<template v-if="store.unread > 0"> · {{ store.unread }}</template>
        </button>
      </div>

      <div class="notif-pop__list">
        <button
          v-for="i in filtered"
          :key="i.id"
          type="button"
          class="notif-row"
          @click="onRow(i)"
        >
          <span v-if="!i.read" class="notif-row__dot" aria-hidden="true" />
          <span class="notif-row__main">
            <span class="notif-row__title">{{ kindTitle(i) }}</span>
            <span class="notif-row__body">{{ bodyText(i) }}</span>
          </span>
          <span class="notif-row__time">{{ timeText(i) }}</span>
        </button>
        <p v-if="filtered.length === 0" class="notif-pop__empty">
          {{ filter === "all" ? t("notify.empty") : t("notify.emptyUnread") }}
        </p>
      </div>
  </div>
</template>

<style scoped>
/* §五 popover 家族（前端标准 §二③）：elevated 面 + 1px line 围边 + 微弱
 * 悬浮影 + 圆角 14；fixed 锚触发钮，进退场由挂载点 Transition 接管 */
.notif-pop {
  position: fixed;
  width: 380px;
  max-width: calc(100vw - 16px);
  border-radius: var(--lw-r-md-plus);
  border: 1px solid var(--lw-line);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-pop);
  padding: 10px 12px;
  z-index: var(--lw-z-pop);
  font-size: 12.5px;
  line-height: 1.45;
}
.notif-pop__head {
  display: flex;
  align-items: center;
  gap: 2px;
  color: var(--lw-ink-1);
}
.notif-pop__title {
  flex: 1 1 auto;
  font-weight: 500;
}
.notif-pop__act {
  display: inline-flex;
  width: var(--lw-ctl);
  height: var(--lw-ctl);
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-xs);
  color: var(--lw-ink-3);
}
.notif-pop__act:hover:not(:disabled) {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.notif-pop__act:disabled {
  opacity: 0.4;
}

/* 轻量两键分段（§3.13）：胶囊内 wash 选中 */
.notif-pop__seg {
  display: inline-flex;
  gap: 2px;
  margin-top: 8px;
  padding: 2px;
  border-radius: var(--lw-r-full);
  background: var(--lw-wash-2);
}
.notif-pop__seg button {
  border-radius: var(--lw-r-full);
  padding: 2px 10px;
  font-size: 11.5px;
  color: var(--lw-ink-3);
}
.notif-pop__seg button:hover {
  color: var(--lw-ink-1);
}
.notif-pop__seg button[data-on] {
  background: var(--lw-bg-elevated);
  color: var(--lw-ink-1);
  box-shadow: var(--lw-shadow-raised);
}

.notif-pop__list {
  margin-top: 6px;
  max-height: min(420px, 50vh);
  overflow-y: auto;
}
.notif-row {
  display: flex;
  align-items: flex-start;
  gap: 8px;
  width: 100%;
  padding: 6px 8px;
  border-radius: var(--lw-r-xs);
  text-align: left;
}
.notif-row:hover {
  background: var(--lw-wash-2);
}
.notif-row__dot {
  width: 6px;
  height: 6px;
  flex: 0 0 auto;
  margin-top: 5px;
  border-radius: var(--lw-r-full);
  background: var(--lw-accent);
}
.notif-row__main {
  min-width: 0;
  flex: 1 1 auto;
  display: flex;
  flex-direction: column;
  gap: 1px;
}
.notif-row__title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12.5px;
  color: var(--lw-ink-2);
}
.notif-row__body {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 11.5px;
  color: var(--lw-ink-3);
}
.notif-row__time {
  flex: 0 0 auto;
  font-size: 11px;
  color: var(--lw-ink-4);
}
.notif-pop__empty {
  padding: 14px 8px;
  text-align: center;
  font-size: 12px;
  color: var(--lw-ink-4);
}
</style>
