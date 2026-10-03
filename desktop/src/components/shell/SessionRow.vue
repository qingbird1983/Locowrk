<script setup lang="ts">
/**
 * 会话行（UI shell 文档 §2.3/§3.6）：28px 行 + 运行/待决状态点 + 行尾
 * `…` 菜单（仅 hover/选中显形）= 重命名 / 导出 / 工作区关联（v2.50
 * §2.3：移入候选 = fsRoots，当前归属置灰打勾；移出）/ 删除（两步确认，
 * 危险项 §3.20 规则）；重命名 = 行内编辑（Enter/失焦提交、Esc 取消、
 * 空串取消）。运行中禁切/禁删由「开发者」危险开关可解（可配置原则
 * §七-16②：禁止项都有开关）——后端忙锁守卫仍在，开关只解锁尝试。
 */
import { computed, nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useApprovalsStore } from "../../stores/approvals";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useShellStore } from "../../stores/shell";
import { useToastStore } from "../../stores/toast";
import type { SessionMeta } from "../../types";
import { workspaceDisplayName } from "../../lib/session-groups";
import { Icon } from "../../lib/icons";
import RowMenu, { type RowMenuItem } from "./RowMenu.vue";

const props = defineProps<{ session: SessionMeta; nested?: boolean }>();
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

/** 工作区候选 = 设置 → 工具 → 工作区目录（后端还会校验，前端只是
 * 把候选收窄到当前配置）。 */
const fsRoots = computed(() => settings.view?.tools.fsRoots ?? []);

const menuItems = computed<RowMenuItem[]>(() => {
  const items: RowMenuItem[] = [
    { id: "rename", label: t("sidebar.menu.rename") },
    { id: "export", label: t("sidebar.menu.export") },
  ];
  if (fsRoots.value.length > 0) {
    items.push({ id: "sep-ws", label: "", separator: true });
    for (const root of fsRoots.value) {
      const current = props.session.workspace === root;
      items.push({
        id: `ws:${root}`,
        label: current
          ? `✓ ${workspaceDisplayName(root)}`
          : t("sidebar.menu.moveInto", { name: workspaceDisplayName(root) }),
        disabled: current,
      });
    }
    if (props.session.workspace) {
      items.push({ id: "ws:clear", label: t("sidebar.menu.moveOut") });
    }
  }
  items.push(
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
  );
  return items;
});

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
  } else if (id.startsWith("ws:")) {
    // 工作区关联（v2.50 §2.3）：ws:clear = 移出，ws:<路径> = 移入
    closeMenu();
    const target = id.slice(3);
    try {
      await chat.associateSession(sid, target === "clear" ? null : target);
      toast.push(
        "ok",
        target === "clear" ? t("toast.movedOut") : t("toast.movedInto"),
      );
    } catch (e) {
      toast.push("danger", t("toast.associateFailed"), String(e));
    }
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
    :class="{ 'session-row--nested': nested }"
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
 * wash-3 填充（禁止左侧色条）；行内缩进让出分组字形宽；行间 2px 垂直
 * 间隙（§7-15：可高亮行的高亮瓦片不得粘连） */
.session-row {
  position: relative;
  display: flex;
  align-items: center;
  min-height: 28px;
  margin-top: 2px;
  border-radius: var(--lw-r-xs);
  padding: 0 4px 0 28px;
}
.session-row:hover {
  background: var(--lw-wash-2);
}
.session-row[data-selected] {
  background: var(--lw-wash-3);
}
/* 项目内的任务行（§2.3 v3.9 issue #9 + 用户裁定）：任务名**不缩进**——
 * 与项目名文字同一左缘（28px，上下左右对齐）；名前 1px 竖向发丝线挂在
 * 项目行文件夹字形中心下方（树形引导线，不接横臂） */
.session-row--nested {
  padding-left: 28px;
}
.session-row--nested::before {
  content: "";
  position: absolute;
  top: 0;
  bottom: 0;
  left: 15px;
  width: 1px;
  background: var(--lw-line-strong);
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
