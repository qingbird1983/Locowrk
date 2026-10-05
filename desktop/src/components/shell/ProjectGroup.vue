<script setup lang="ts">
/**
 * 项目分组（UI shell doc §2.3 v3.9，issue #7⑧⑨）：28px 项目行 = 文件夹
 * 字形双态（折叠 = 闭文件夹 / 展开 = 开文件夹，v3.9 不再画前置 chevron）
 * + 13px 项目名 + 可选 6px 运行点；hover 显形右对齐「⋯ / ＋」——⋯ = 项目
 * 菜单（新建任务 / 在文件夹中显示 / 移除项目 danger），＋ = 该项目下新建
 * 任务（新会话带归属，首聊自动关联，§3.6）。点击行体 = 折叠/展开（折叠
 * 态持久化在父层）。任务行 = SessionRow（nested：发丝线 + 状态点）。
 */
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useShellStore } from "../../stores/shell";
import { useToastStore } from "../../stores/toast";
import { revealInExplorer } from "../../lib/api";
import type { WorkspaceGroup } from "../../lib/session-groups";
import { Icon } from "../../lib/icons";
import HoverTip from "../settings/HoverTip.vue";
import RowMenu, { type RowMenuItem } from "./RowMenu.vue";
import SessionRow from "./SessionRow.vue";

const props = defineProps<{ group: WorkspaceGroup; collapsed: boolean }>();
const emit = defineEmits<{ toggle: [] }>();

const { t } = useI18n();
const chat = useChatStore();
const settings = useSettingsStore();
const shell = useShellStore();
const toast = useToastStore();

/** ＋ = 在该项目下新建任务（§3.6 v3.9）：前端新会话记账归属，首聊后
 * session_associate 落旁车（运行中禁新建，与顶栏/⌘K 同一守卫）。 */
function newTask() {
  if (chat.running) return;
  chat.newSession(props.group.key);
  shell.setDestination("chat");
}

// ---- 项目菜单（⋯，§3.20 形态）----
const menu = ref<{ x: number; y: number } | null>(null);

const menuItems = (): RowMenuItem[] => [
  { id: "newTask", label: t("sidebar.projectMenu.newTask") },
  { id: "reveal", label: t("sidebar.projectMenu.reveal") },
  { id: "sep", label: "", separator: true },
  { id: "remove", label: t("sidebar.projectMenu.remove"), danger: true },
];

function openMenu(e: MouseEvent) {
  const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
  menu.value = { x: r.right + 2, y: r.bottom + 2 };
}

function closeMenu() {
  menu.value = null;
}

async function onMenuSelect(id: string) {
  closeMenu();
  if (id === "newTask") {
    newTask();
  } else if (id === "reveal") {
    try {
      await revealInExplorer(props.group.key);
    } catch (e) {
      toast.push("danger", t("toast.revealFailed"), String(e));
    }
  } else if (id === "remove") {
    // 移除项目 = 从 fsRoots 移除（会话与既有归属不动，组照常显示）
    const rest = (settings.view?.tools.fsRoots ?? []).filter(
      (r) => r !== props.group.key,
    );
    try {
      await settings.saveTools({ fsRoots: rest });
      toast.push("ok", t("toast.projectRemoved"));
    } catch (e) {
      toast.push("danger", t("toast.projectRemoveFailed"), String(e));
    }
  }
}
</script>

<template>
  <div>
    <button
      type="button"
      class="ws-head"
      :aria-expanded="!collapsed"
      @click="emit('toggle')"
    >
      <Icon
        :name="collapsed ? 'folder' : 'folderOpen'"
        :size="14"
        aria-hidden="true"
      />
      <HoverTip class="flex-auto min-w-0" :text="group.key" :label="group.key">
        <span class="ws-head__name">{{ group.name }}</span>
      </HoverTip>
      <HoverTip v-if="group.running" :text="t('status.running')">
        <span
          class="ws-head__dot"
          role="img"
          :aria-label="t('status.running')"
        />
      </HoverTip>
      <!-- hover 显形动作（issue #8）：＋ 新建任务 / ⋯ 项目菜单；不冒泡 -->
      <span class="ws-head__actions" @click.stop>
        <HoverTip :text="t('sidebar.projectMenu.newTask')">
          <button
            type="button"
            class="ws-head__btn"
            :aria-label="t('sidebar.projectMenu.newTask')"
            @click="newTask"
          >
            <Icon name="plus" :size="13" aria-hidden="true" />
          </button>
        </HoverTip>
        <HoverTip :text="t('sidebar.projectMenu.more')">
          <button
            type="button"
            class="ws-head__btn"
            :aria-label="t('sidebar.projectMenu.more')"
            @click="openMenu"
          >
            <Icon name="dots" :size="13" aria-hidden="true" />
          </button>
        </HoverTip>
      </span>
    </button>
    <template v-if="!collapsed">
      <SessionRow
        v-for="s in group.items"
        :key="s.id"
        :session="s"
        nested
      />
    </template>
    <!-- 菜单进退场（标准 §二①）：Teleport 出 aside 裁剪 + Transition 接管 -->
    <Teleport to="body">
      <Transition name="lw-pop-down">
        <RowMenu
          v-if="menu"
          :items="menuItems()"
          :x="menu.x"
          :y="menu.y"
          @select="onMenuSelect"
          @close="closeMenu"
        />
      </Transition>
    </Teleport>
  </div>
</template>

<style scoped>
/* 项目行（§2.3 v3.9）：28px 行、行间 2px 间隙（§7-15）；文件夹字形双态
 * 即折叠状态指示（无 chevron，§6.6 禁旋转令照旧） */
.ws-head {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-6);
  width: 100%;
  min-height: 28px;
  margin-top: var(--lw-gap-2);
  padding: 0 var(--lw-gap-4) 0 var(--lw-gap-8);
  border-radius: var(--lw-r-xs);
  text-align: left;
}
.ws-head:hover {
  background: var(--lw-wash-2);
}
.ws-head__name {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--lw-fs-13);
  color: var(--lw-ink-2);
}
.ws-head__dot {
  width: 6px;
  height: 6px;
  flex: 0 0 auto;
  border-radius: var(--lw-r-full);
  background: var(--lw-ok);
}
.ws-head__actions {
  display: inline-flex;
  align-items: center;
  gap: 1px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值1,§二⑬) */
  flex: 0 0 auto;
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease-out);
}
.ws-head:hover .ws-head__actions,
.ws-head__actions:focus-within {
  opacity: 1;
}
.ws-head__btn {
  display: inline-flex;
  width: 20px;
  height: 20px;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-xs);
  color: var(--lw-ink-4);
}
.ws-head__btn:hover {
  background: var(--lw-wash-3);
  color: var(--lw-ink-1);
}
</style>
