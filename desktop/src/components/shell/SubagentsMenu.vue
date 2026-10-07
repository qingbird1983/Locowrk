<script setup lang="ts">
/**
 * 顶栏子代理入口 + 状态面板（UI shell doc §2.1 ③，v4.18）：标题后的
 * bot 字形钮 + 数量徽章——**会话有 subagent_run 活动才显示，没有就
 * 隐藏**（用户裁定；不渲染空态）。点开下弹面板列本会话已派发的子代理
 * （任务截断 + 状态：执行中 = 脉冲点 / 已完成 / 步数到顶 / 超时 / 失败），
 * 行点击开右栏转录页签（§3.16）——`child_session_id` 在场才可点，运行中
 * 无 id 如实不可点（与 §3.10 明细「查看完整对话」入口同一门槛）。
 * 数据源 = 当前会话 items 里的 subagent_run 工具行（与过程行/L1 极简
 * 摘要同源，零新链路）；弹层 = §五 popover 家族（teleport body + fixed
 * 锚钮下弹 + 外点收口；Esc 不抢 composer 的 Esc 链，同 §3.13 先例）。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore, type ToolItem } from "../../stores/chat";
import { useWorkPanelStore } from "../../stores/work-panel";
import { Icon } from "../../lib/icons";
import HoverTip from "../settings/HoverTip.vue";

const { t } = useI18n();
const chat = useChatStore();
const workPanel = useWorkPanelStore();

const open = ref(false);
const btn = ref<HTMLButtonElement | null>(null);
const root = ref<HTMLElement | null>(null);
const pos = ref({ left: 0, top: 0 });

/** 本会话的 subagent_run 工具行（items 时序 = 派发顺序）。 */
const runs = computed(() =>
  chat.items.filter(
    (i): i is ToolItem => i.kind === "tool" && i.tool === "subagent_run",
  ),
);

interface SubagentRow {
  key: string;
  task: string;
  label: string;
  state: "running" | "ok" | "danger";
  childId: string | null;
}

function asRecord(v: unknown): Record<string, unknown> {
  return v !== null && typeof v === "object"
    ? (v as Record<string, unknown>)
    : {};
}

/** 状态词与过程行/L1 摘要同源（tool.running / tool.failed / tool.value.*）。 */
function rowOf(item: ToolItem): SubagentRow {
  const a = asRecord(item.args);
  const o = asRecord(item.output);
  const prompt = typeof a.prompt === "string" ? a.prompt.trim() : "";
  const task = prompt || t("topbar.subagents");
  if (item.pending) {
    return {
      key: item.callId,
      task,
      label: t("tool.running"),
      state: "running",
      childId: null,
    };
  }
  if (item.ok === false) {
    return {
      key: item.callId,
      task,
      label: t("tool.failed"),
      state: "danger",
      childId: null,
    };
  }
  const s = typeof o.status === "string" ? o.status : "completed";
  const label =
    s === "budget_exhausted"
      ? t("tool.value.budgetExhausted")
      : s === "timeout"
        ? t("tool.value.timeout")
        : t("tool.value.completed");
  const childId =
    typeof o.child_session_id === "string" && o.child_session_id
      ? o.child_session_id
      : null;
  return {
    key: item.callId,
    task,
    label,
    state: s === "completed" ? "ok" : "danger",
    childId,
  };
}

const rows = computed(() => runs.value.map(rowOf));

function toggle() {
  if (open.value) {
    open.value = false;
    return;
  }
  const rect = btn.value?.getBoundingClientRect();
  if (rect) {
    pos.value = {
      left: Math.max(8, Math.min(rect.left, window.innerWidth - 360 - 8)),
      top: rect.bottom + 6,
    };
  }
  open.value = true;
}

/** 收束的子代理才开转录页签（与明细入口同门槛；运行中按钮已 disabled）。 */
function onRow(r: SubagentRow) {
  if (!r.childId) return;
  workPanel.openSubTab(r.childId, r.task);
  open.value = false;
}

function onDocPointerDown(e: PointerEvent) {
  if (!open.value) return;
  const target = e.target as Node;
  if (root.value?.contains(target) || btn.value?.contains(target)) return;
  open.value = false;
}

onMounted(() => document.addEventListener("pointerdown", onDocPointerDown, true));
onBeforeUnmount(() =>
  document.removeEventListener("pointerdown", onDocPointerDown, true),
);
</script>

<template>
  <HoverTip v-if="runs.length > 0" :text="t('topbar.subagents')">
    <button
      ref="btn"
      class="sub-btn"
      type="button"
      :aria-label="t('topbar.subagents')"
      :aria-expanded="open"
      @click="toggle"
    >
      <Icon name="bot" :size="14" aria-hidden="true" />
      <span class="sub-btn__count">{{ runs.length }}</span>
    </button>
  </HoverTip>
  <Teleport to="body">
    <Transition name="lw-pop-down">
      <div
        v-if="open"
        ref="root"
        class="sub-pop"
        role="dialog"
        :aria-label="t('topbar.subagents')"
        :style="{ left: `${pos.left}px`, top: `${pos.top}px` }"
      >
        <div class="sub-pop__head">
          <span class="sub-pop__title">{{ t("topbar.subagents") }}</span>
          <span class="sub-pop__count">{{ rows.length }}</span>
        </div>
        <div class="sub-pop__list">
          <button
            v-for="r in rows"
            :key="r.key"
            type="button"
            class="sub-row"
            :class="{ 'is-link': !!r.childId }"
            :disabled="!r.childId"
            :title="r.childId ? t('panel.viewTranscript') : undefined"
            @click="onRow(r)"
          >
            <span
              class="sub-row__dot"
              :class="[`is-${r.state}`, { 'lw-pulse': r.state === 'running' }]"
              aria-hidden="true"
            />
            <span class="sub-row__task">{{ r.task }}</span>
            <span class="sub-row__status" :class="`is-${r.state}`">{{
              r.label
            }}</span>
          </button>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
/* 触发钮（§2.1 ③）：与顶栏其他 ctl 同量级（28px 高、r-md、wash-2 hover），
 * bot 字形 + 数量徽章；无子代理活动时整个不渲染（v-if 在根） */
.sub-btn {
  display: inline-flex;
  align-items: center;
  gap: var(--lw-gap-2);
  height: var(--lw-ctl);
  padding: 0 var(--lw-gap-6);
  flex: 0 0 auto;
  border-radius: var(--lw-r-md);
  color: var(--lw-ink-2);
}
.sub-btn:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.sub-btn__count {
  font-size: var(--lw-fs-11);
  line-height: 1;
  color: var(--lw-ink-3);
}

/* §五 popover 家族：elevated 面 + 1px line 围边 + 微弱悬浮影 + 圆角 14；
 * fixed 锚触发钮下弹，进退场由 lw-pop-down Transition 接管 */
.sub-pop {
  position: fixed;
  width: 360px;
  max-width: calc(100vw - 16px);
  border-radius: var(--lw-r-md-plus);
  border: 1px solid var(--lw-line);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-pop);
  padding: var(--lw-gap-10) var(--lw-gap-12);
  z-index: var(--lw-z-pop);
  font-size: var(--lw-fs-12half);
  line-height: 1.45;
}
.sub-pop__head {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-2);
  color: var(--lw-ink-1);
}
.sub-pop__title {
  flex: 1 1 auto;
  font-weight: 500;
}
.sub-pop__count {
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-3);
}
.sub-pop__list {
  margin-top: var(--lw-gap-6);
  max-height: min(420px, 50vh);
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: var(--lw-gap-2);
}
.sub-row {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
  width: 100%;
  padding: var(--lw-gap-6) var(--lw-gap-8);
  border-radius: var(--lw-r-xs);
  text-align: left;
}
.sub-row.is-link:hover {
  background: var(--lw-wash-2);
}
.sub-row:disabled {
  cursor: default;
}
.sub-row__dot {
  width: 6px;
  height: 6px;
  flex: 0 0 auto;
  border-radius: var(--lw-r-full);
  background: var(--lw-accent);
}
.sub-row__dot.is-ok {
  background: var(--lw-ok);
}
.sub-row__dot.is-danger {
  background: var(--lw-danger);
}
.sub-row__task {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: var(--lw-ink-2);
}
.sub-row.is-link .sub-row__task:hover {
  color: var(--lw-ink-1);
}
.sub-row__status {
  flex: 0 0 auto;
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-3);
}
.sub-row__status.is-ok {
  color: var(--lw-ok);
}
.sub-row__status.is-danger {
  color: var(--lw-danger);
}
</style>
