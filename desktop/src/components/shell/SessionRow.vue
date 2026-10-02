<script setup lang="ts">
/**
 * 会话行（UI shell 文档 §2.3/§3.6）：28px 行 + 运行/待决状态点 + 行尾
 * `…` 菜单（仅 hover/选中显形）= 重命名 / 导出 / 删除（两步确认，危险
 * 项 §3.20 规则）；重命名 = 行内编辑（Enter/失焦提交、Esc 取消、空串
 * 取消）。运行中禁切/禁删由「开发者」危险开关可解（可配置原则 §七-16②：
 * 禁止项都有开关）——后端忙锁守卫仍在，开关只解锁尝试。
 */
import { computed, nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useApprovalsStore } from "../../stores/approvals";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useShellStore } from "../../stores/shell";
import { useToastStore } from "../../stores/toast";
import type { SessionMeta } from "../../types";
import { Icon } from "../../lib/icons";
import RowMenu, { type RowMenuItem } from "./RowMenu.vue";

const props = defineProps<{ session: SessionMeta }>();
const { t } = useI18n();
const chat = useChatStore();
const shell = useShellStore();
const approvals = useApprovalsStore();
const settings = useSettingsStore();
const toast = useToastStore();

const isCurrent = computed(() => props.session.id === chat.sessionId);
/** 禁止项开关（开发者分区，缺省关 = 保持禁切语义）。 */
const switchLocked = computed(
  () =>
    chat.running &&
    !isCurrent.value &&
    !settings.view?.developer.allowSwitchRunningSession,
);
const deleteDisabled = computed(
  () =>
    isCurrent.value &&
    chat.running &&
    !settings.view?.developer.allowDeleteRunningSession,
);

function open() {
  if (switchLocked.value) return;
  void chat.switchSession(props.session.id);
  shell.setDestination("chat");
}

// ---- 行尾 `…` 菜单（§3.20）----
const menu = ref<{ x: number; y: number } | null>(null);
const confirmingDelete = ref(false);
const moreBtn = ref<HTMLButtonElement | null>(null);

const menuItems = computed<RowMenuItem[]>(() => [
  { id: "rename", label: t("sidebar.menu.rename") },
  { id: "export", label: t("sidebar.menu.export") },
  { id: "sep", label: "", separator: true },
  {
    id: "delete",
    label: confirmingDelete.value
      ? t("sidebar.menu.confirmDelete")
      : t("sidebar.menu.delete"),
    danger: true,
    disabled: deleteDisabled.value,
    keepOpen: true,
  },
]);

function openMenu(e: MouseEvent) {
  confirmingDelete.value = false;
  const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
  menu.value = { x: r.right + 2, y: r.bottom + 2 };
}

function closeMenu() {
  menu.value = null;
  confirmingDelete.value = false;
  moreBtn.value?.focus(); // §3.20：Esc/选择后焦点还给触发控件
}

async function onMenuSelect(id: string) {
  const sid = props.session.id;
  if (id === "rename") {
    closeMenu();
    void startRename();
  } else if (id === "export") {
    closeMenu();
    try {
      const path = await chat.exportSession(sid);
      if (path) toast.push("ok", t("toast.exported"), path);
    } catch (e) {
      toast.push("danger", t("toast.exportFailed"), String(e));
    }
  } else if (id === "delete") {
    if (!confirmingDelete.value) {
      // 两步确认第一步：菜单保持开，条目变「确认删除？」
      confirmingDelete.value = true;
      return;
    }
    closeMenu();
    try {
      await chat.deleteSession(sid);
      toast.push("ok", t("toast.deleted"));
    } catch (e) {
      toast.push("danger", t("toast.deleteFailed"), String(e));
    }
  }
}

// ---- 行内重命名 ----
const renaming = ref(false);
const draft = ref("");
const renameInput = ref<HTMLInputElement | null>(null);

async function startRename() {
  draft.value = props.session.title ?? "";
  renaming.value = true;
  await nextTick();
  renameInput.value?.focus();
  renameInput.value?.select();
}

async function commitRename() {
  if (!renaming.value) return;
  renaming.value = false;
  const title = draft.value.trim();
  if (!title || title === (props.session.title ?? "")) return;
  try {
    await chat.renameSession(props.session.id, title);
  } catch (e) {
    toast.push("danger", t("toast.renameFailed"), String(e));
  }
}
</script>

<template>
  <div
    class="session-row"
    :data-selected="isCurrent || undefined"
    :title="session.title ?? session.id"
  >
    <input
      v-if="renaming"
      ref="renameInput"
      v-model="draft"
      type="text"
      class="session-row__edit"
      :aria-label="t('sidebar.menu.rename')"
      @keydown.enter.prevent="commitRename"
      @keydown.esc.prevent="renaming = false"
      @blur="commitRename"
    />
    <template v-else>
      <button
        class="session-row__main"
        type="button"
        :class="{ 'cursor-not-allowed opacity-40': switchLocked }"
        :aria-disabled="switchLocked"
        :title="
          switchLocked ? t('sidebar.lockedWhileRunning') : (session.title ?? session.id)
        "
        @click="open"
      >
        <span
          v-if="chat.running && isCurrent"
          class="dot dot--pulse"
          :title="t('status.running')"
        />
        <!-- §3.17 卡不在视野内的兜底：本会话有审批未决 → 待决标记 -->
        <span
          v-else-if="approvals.pending.length > 0 && isCurrent"
          class="dot dot--danger"
          role="img"
          :aria-label="t('approval.pendingBadge')"
          :title="t('approval.pendingBadge')"
        />
        <span class="session-row__title">
          {{ session.title ?? t("sidebar.untitled") }}
        </span>
      </button>
      <button
        ref="moreBtn"
        type="button"
        class="session-row__more"
        :aria-label="t('sidebar.menu.more')"
        :title="t('sidebar.menu.more')"
        @click="openMenu"
      >
        <Icon name="dots" :size="13" aria-hidden="true" />
      </button>
    </template>
    <RowMenu
      v-if="menu"
      :items="menuItems"
      :x="menu.x"
      :y="menu.y"
      @select="onMenuSelect"
      @close="closeMenu"
    />
  </div>
</template>

<style scoped>
/* 会话行（§2.3）：28px、圆角 8、0 8px 内边距、13px、超长省略；选中 =
 * wash-3 填充（禁止左侧色条）；行内缩进让出分组字形宽度 */
.session-row {
  display: flex;
  align-items: center;
  min-height: 28px;
  border-radius: var(--lw-r-xs);
  padding: 0 4px 0 28px;
}
.session-row:hover {
  background: var(--lw-wash-2);
}
.session-row[data-selected] {
  background: var(--lw-wash-3);
}
.session-row__main {
  display: flex;
  min-width: 0;
  flex: 1 1 auto;
  align-items: center;
  gap: 6px;
  text-align: left;
}
.session-row__title {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
  color: var(--lw-ink-2);
}
.session-row[data-selected] .session-row__title {
  color: var(--lw-ink-1);
}
/* 行尾 … 按钮（§2.3）：仅 hover / 选中显形 */
.session-row__more {
  display: inline-flex;
  width: 20px;
  height: 20px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-xs);
  color: var(--lw-ink-4);
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease-out);
}
.session-row:hover .session-row__more,
.session-row[data-selected] .session-row__more,
.session-row__more:focus-visible {
  opacity: 1;
}
.session-row__more:hover {
  background: var(--lw-wash-3);
  color: var(--lw-ink-1);
}
.session-row__more:focus-visible {
  outline: none;
}
/* 行内重命名：与行同高同字形 */
.session-row__edit {
  width: 100%;
  min-width: 0;
  height: 22px;
  padding: 0 6px;
  border-radius: var(--lw-r-xs);
  background: var(--lw-bg-inset);
  font-size: 12.5px;
  color: var(--lw-ink-1);
}
/* 运行指示点（§6.6 ⑧：唯一常驻动画）+ 4px 状态点 */
.dot {
  width: 4px;
  height: 4px;
  flex: 0 0 auto;
  border-radius: var(--lw-r-full);
}
.dot--pulse {
  background: var(--lw-ok);
  animation: lw-pulse 1s ease-in-out infinite;
}
.dot--danger {
  background: var(--lw-danger);
}
</style>
