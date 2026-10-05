<script setup lang="ts">
/**
 * 会话树（UI shell 文档 §2.3 v3.9 项目接管）：**侧栏只保留项目工作区**
 * （用户裁定）——组头行 = 标签 + 下三角（点击折叠/展开所有项目，字形随
 * 状态换形）+ hover 显形「筛选 / 新建项目」；项目行在 ProjectGroup（文件
 * 夹双态 + ⋯/＋）；无归属会话不进树（⌘K 面板照列全部）。折叠键 =
 * workspace 存储路径，随侧栏偏好持久化。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useToastStore } from "../../stores/toast";
import { pickDirectory } from "../../lib/api";
import type { SessionMeta } from "../../types";
import { groupSessions } from "../../lib/session-groups";
import { loadCollapsedGroups, saveCollapsedGroups } from "../../lib/sidebar-preference";
import { Icon } from "../../lib/icons";
import HoverTip from "../settings/HoverTip.vue";
import RowMenu, { type RowMenuItem } from "./RowMenu.vue";
import ProjectGroup from "./ProjectGroup.vue";

const props = defineProps<{ sessions: SessionMeta[]; fsRoots: string[] }>();
const { t } = useI18n();
const chat = useChatStore();
const settings = useSettingsStore();
const toast = useToastStore();

const runningId = computed(() => (chat.running ? chat.sessionId : null));

// 「只看进行中」筛选（issue #6 筛选图标的最小诚实实现：纯前端过滤）
const onlyRunning = ref(false);
const tree = computed(() => {
  if (!onlyRunning.value) {
    return groupSessions(props.sessions, runningId.value, props.fsRoots);
  }
  const visible = props.sessions.filter((s) => s.id === runningId.value);
  return groupSessions(visible, runningId.value, props.fsRoots, {
    includeEmpty: false,
  });
});

// 分组头折叠（§2.3）：键 = workspace 存储路径，随偏好持久化
const collapsed = ref(loadCollapsedGroups());

function toggle(key: string) {
  const next = new Set(collapsed.value);
  if (!next.delete(key)) next.add(key);
  collapsed.value = next;
  saveCollapsedGroups(next);
}

/** 下三角（issue #6）：任一项目展开 → 折叠所有；已全折叠 → 全部展开。 */
const allCollapsed = computed(
  () => tree.value.workspaces.length > 0 && tree.value.workspaces.every((w) => collapsed.value.has(w.key)),
);

function toggleAll() {
  const next = new Set(collapsed.value);
  if (allCollapsed.value) {
    for (const w of tree.value.workspaces) next.delete(w.key);
  } else {
    for (const w of tree.value.workspaces) next.add(w.key);
  }
  collapsed.value = next;
  saveCollapsedGroups(next);
}

// ---- 组头行右缘动作（issue #6）：筛选菜单 + 新建项目 ----
const filterMenu = ref<{ x: number; y: number } | null>(null);

const filterItems = computed<RowMenuItem[]>(() => [
  { id: "all", label: t("sidebar.filterAll") },
  {
    id: "running",
    label: onlyRunning.value ? `✓ ${t("sidebar.filterRunning")}` : t("sidebar.filterRunning"),
  },
]);

function openFilterMenu(e: MouseEvent) {
  const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
  filterMenu.value = { x: r.right + 2, y: r.bottom + 2 };
}

function onFilterSelect(id: string) {
  filterMenu.value = null;
  onlyRunning.value = id === "running";
}

/** 新建项目 = 原生目录选择 → 追加进设置 fsRoots（保存即生效，§2.3）。 */
async function addProject() {
  const dir = await pickDirectory();
  if (!dir || props.fsRoots.includes(dir)) return;
  try {
    await settings.saveTools({ fsRoots: [...props.fsRoots, dir] });
    toast.push("ok", t("toast.projectAdded"));
  } catch (e) {
    toast.push("danger", t("toast.projectAddFailed"), String(e));
  }
}
</script>

<template>
  <div>
    <!-- 「项目」组头行（issue #6）：下三角左对齐紧跟文字；筛选/新建右对齐
         hover 显形 -->
    <div class="proj-head">
      <HoverTip
        :text="t('sidebar.toggleAllProjects')"
        :label="t('sidebar.toggleAllProjects')"
      >
        <button
          type="button"
          class="proj-head__label"
          :aria-expanded="!allCollapsed"
          @click="toggleAll"
        >
          {{ t("sidebar.projects") }}
          <Icon
            :name="allCollapsed ? 'chevronRight' : 'chevronDown'"
            :size="12"
            aria-hidden="true"
          />
        </button>
      </HoverTip>
      <span class="proj-head__actions">
        <HoverTip :text="t('sidebar.filter')">
          <button
            type="button"
            class="proj-head__btn"
            :aria-label="t('sidebar.filter')"
            @click="openFilterMenu"
          >
            <Icon name="filter" :size="13" aria-hidden="true" />
          </button>
        </HoverTip>
        <HoverTip :text="t('sidebar.addProject')">
          <button
            type="button"
            class="proj-head__btn"
            :aria-label="t('sidebar.addProject')"
            @click="addProject"
          >
            <Icon name="plus" :size="13" aria-hidden="true" />
          </button>
        </HoverTip>
      </span>
      <!-- 菜单进退场（标准 §二①）：Teleport 出 aside 裁剪 + Transition 接管 -->
      <Teleport to="body">
        <Transition name="lw-pop-down">
          <RowMenu
            v-if="filterMenu"
            :items="filterItems"
            :x="filterMenu.x"
            :y="filterMenu.y"
            @select="onFilterSelect"
            @close="filterMenu = null"
          />
        </Transition>
      </Teleport>
    </div>

    <ProjectGroup
      v-for="w in tree.workspaces"
      :key="w.key"
      :group="w"
      :collapsed="collapsed.has(w.key)"
      @toggle="toggle(w.key)"
    />
  </div>
</template>

<style scoped>
/* 项目组头行（issue #6）：11.5px 标签 + 下三角；右缘动作 hover 显形 */
.proj-head {
  display: flex;
  align-items: center;
  padding: 10px 4px 2px 8px;
}
.proj-head__label {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  min-height: 20px;
  padding: 0 4px;
  border-radius: var(--lw-r-xs);
  font-size: 11.5px;
  color: var(--lw-ink-4);
}
.proj-head__label:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-2);
}
.proj-head__actions {
  display: inline-flex;
  align-items: center;
  gap: 1px;
  margin-left: auto;
  opacity: 0;
  transition: opacity var(--dur-fast) var(--ease-out);
}
.proj-head:hover .proj-head__actions,
.proj-head__actions:focus-within {
  opacity: 1;
}
.proj-head__btn {
  display: inline-flex;
  width: 20px;
  height: 20px;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-xs);
  color: var(--lw-ink-4);
}
.proj-head__btn:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
</style>
