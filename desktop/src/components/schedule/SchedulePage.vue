<script setup lang="ts">
/**
 * 定时任务目的地页（UI shell doc §3.35，v2.57）：⑭ 调度库的读者与裁决
 * 面。行 = 徽章（一次性/每 N）+ 标题 + prompt 摘要 + 目标会话 + 下次触发
 * 相对时间 + 行尾删除（两步确认，§3.6 同款）；空态引导（创建入口是模型
 * 工具 schedule_create，页面不做编辑器——§3.35 宁缺勿假）。数据全部经
 * schedule_list / schedule_delete IPC，本页不写业务规则。
 */
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { scheduleDelete, scheduleList } from "../../lib/api";
import { useChatStore } from "../../stores/chat";
import { clip } from "../../lib/format";
import { Icon } from "../../lib/icons";
import type { SessionSchedule } from "../../types";

const { t } = useI18n();
const chat = useChatStore();

const rows = ref<SessionSchedule[] | null>(null); // null = 加载中
const loadError = ref("");
/** 删除两步确认中的行 id（首点变「确认删除？」）。 */
const confirmingDelete = ref<string | null>(null);

const isEmpty = computed(() => (rows.value?.length ?? 0) === 0);

async function load() {
  loadError.value = "";
  try {
    rows.value = await scheduleList();
  } catch (e) {
    loadError.value = String(e);
    rows.value = [];
  }
}

onMounted(load);

/** 目标会话名：侧栏同源标题；会话已删显示 id（如实）。 */
function sessionName(id: string): string {
  return chat.sessions.find((s) => s.id === id)?.title ?? id;
}

/** 下次触发的呈现：未来 = 本地时间；逾期 = 等待会话空闲/顺延中（引擎
 * 让位机制的真实状态，不虚构「已跳过」）。 */
function nextFireText(row: SessionSchedule): string {
  if (row.runAtMs > Date.now()) {
    const d = new Date(row.runAtMs);
    const hh = String(d.getHours()).padStart(2, "0");
    const mm = String(d.getMinutes()).padStart(2, "0");
    const sameDay = new Date().toDateString() === d.toDateString();
    return sameDay ? `${hh}:${mm}` : `${d.getMonth() + 1}/${d.getDate()} ${hh}:${mm}`;
  }
  return t("schedule.waiting");
}

/** 周期徽章：分钟 / 小时两档（interval_secs 上游无上限，不发明更大单位）。 */
function badgeText(row: SessionSchedule): string {
  if (row.kind === "once") return t("schedule.once");
  const secs = row.intervalSecs ?? 0;
  return secs % 3600 === 0 && secs >= 3600
    ? t("schedule.everyHours", { n: secs / 3600 })
    : t("schedule.everyMinutes", { n: Math.max(1, Math.round(secs / 60)) });
}

async function onDelete(row: SessionSchedule) {
  if (confirmingDelete.value !== row.id) {
    confirmingDelete.value = row.id;
    return;
  }
  confirmingDelete.value = null;
  await scheduleDelete(row.id).catch(() => undefined);
  await load();
}
</script>

<template>
  <div class="px-6 py-4">
    <p v-if="loadError" class="mb-3 text-[12px] text-danger">{{ loadError }}</p>

    <!-- 空态（§3.35）：一行引导，不发明「新建任务」按钮 -->
    <div v-if="isEmpty" class="schedule-empty">
      <Icon name="clock" :size="20" class="text-ink-4" />
      <p>{{ t("schedule.empty") }}</p>
    </div>

    <ul v-else class="flex flex-col gap-1.5" :aria-label="t('nav.schedule')">
      <li
        v-for="row in rows"
        :key="row.id"
        class="schedule-row"
        data-testid="schedule-row"
      >
        <div class="min-w-0 flex-1">
          <div class="flex min-w-0 items-center gap-2">
            <span class="schedule-badge">{{ badgeText(row) }}</span>
            <span class="truncate text-[13px] text-ink-1">{{ row.title || clip(row.prompt, 40) }}</span>
            <span
              v-if="row.deferrals > 0"
              class="shrink-0 text-[11px] text-warn"
              :title="t('schedule.deferredHint', { n: row.deferrals })"
            >
              {{ t("schedule.deferred", { n: row.deferrals }) }}
            </span>
          </div>
          <p class="schedule-prompt">{{ row.prompt }}</p>
          <p class="schedule-meta">
            <span class="text-ink-4">{{ t("schedule.target") }}</span>
            <span class="text-ink-3">{{ sessionName(row.sessionId) }}</span>
            <span class="text-ink-5">·</span>
            <span class="text-ink-4">{{ t("schedule.next") }}</span>
            <span class="text-ink-3">{{ nextFireText(row) }}</span>
          </p>
        </div>
        <button
          class="row-ctl"
          type="button"
          :class="{ 'row-ctl--danger': confirmingDelete === row.id }"
          :title="confirmingDelete === row.id ? t('schedule.confirmDelete') : t('schedule.delete')"
          @click="onDelete(row)"
          @blur="confirmingDelete = null"
        >
          {{ confirmingDelete === row.id ? t("schedule.confirmDelete") : t("schedule.delete") }}
        </button>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.schedule-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 10px;
  padding: 72px 0;
  text-align: center;
  font-size: 12.5px;
  color: var(--lw-ink-4);
}
.schedule-row {
  display: flex;
  align-items: center;
  gap: 12px;
  border-radius: var(--lw-r-md);
  background: var(--lw-wash-1);
  padding: 10px 14px;
}
.schedule-badge {
  flex: 0 0 auto;
  border-radius: var(--lw-r-full);
  background: var(--lw-wash-3);
  padding: 1px 8px;
  font-size: 11px;
  color: var(--lw-ink-3);
}
.schedule-prompt {
  display: -webkit-box;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
  overflow: hidden;
  margin-top: 2px;
  font-size: 12.5px;
  line-height: 1.45;
  color: var(--lw-ink-3);
}
.schedule-meta {
  display: flex;
  align-items: center;
  gap: 4px;
  margin-top: 4px;
  font-size: 11.5px;
}
.row-ctl {
  flex: 0 0 auto;
  border-radius: var(--lw-r-xs);
  padding: 3px 8px;
  font-size: 11.5px;
  color: var(--lw-ink-4);
}
.row-ctl:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.row-ctl--danger,
.row-ctl--danger:hover {
  color: var(--lw-danger);
}
</style>
