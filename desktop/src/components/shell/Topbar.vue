<script setup lang="ts">
/**
 * 会话顶栏（UI shell doc §2.1 ③ / §2.2）：46px 带中段。折叠按钮的 lead
 * 槽恒挂（展开时宽 0——零宽导致标题零跳变，§2.2-6）；标题段是拖拽区；
 * 右端窗口控制 120px 车道按 §2.2-5 单一负责。v3.9（issue #3⑩）：
 * 新建/搜索钮移除（侧栏顶部动作区已有，重复）；工作面板开关首页空态下
 * 隐藏、不可展开（§2.4）。v2.60 修复批：①项目归属钮移进顶栏——标题前
 * 纯图标（WorkspacePicker iconOnly，菜单下弹），composer 底行只剩首页
 * 形态；②标题固定宽度上限，放不下尾部省略；③面板开关去掉 wash 底色，
 * 且面板开合不再移位（padding-right 恒定预留车道）。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { useShellStore } from "../../stores/shell";
import { useWorkPanelStore } from "../../stores/work-panel";
import { Icon } from "../../lib/icons";
import WorkspacePicker from "../chat/WorkspacePicker.vue";

const { t } = useI18n();
const chat = useChatStore();
const shell = useShellStore();
const workPanel = useWorkPanelStore();

/** 首页空态（§3.14 v3.9）：无标题、面板开关隐藏。 */
const isHome = computed(() => chat.items.length === 0);

const title = computed(() => {
  if (isHome.value) return "";
  return (
    chat.sessions.find((s) => s.id === chat.sessionId)?.title ??
    t("sidebar.untitled")
  );
});
</script>

<template>
  <header
    class="relative z-10 flex h-[var(--lw-toolbar-h)] shrink-0 items-center bg-page pl-3"
    style="padding-right: var(--lw-wincontrols)"
    data-tauri-drag-region
  >
    <!-- lead 槽：恒在，展开时宽 0（过渡与侧栏同步，标题不跳变） -->
    <div
      class="lead flex items-center overflow-hidden"
      :class="shell.sidebarCollapsed ? 'lead--open' : ''"
      :aria-hidden="!shell.sidebarCollapsed"
    >
      <button
        class="ctl"
        type="button"
        :title="t('sidebar.expand')"
        :aria-label="t('sidebar.expand')"
        :tabindex="shell.sidebarCollapsed ? 0 : -1"
        @click="shell.toggleSidebar()"
      >
        <Icon name="sidebar" :size="14" aria-hidden="true" />
      </button>
    </div>

    <!-- 标题段（拖拽区）：项目归属图标（v2.60，会话态）→ 运行点 → 标题。
         标题固定宽度上限：放不下尾部省略（v2.60 修复批） -->
    <div
      class="flex min-w-0 flex-1 items-center gap-1.5 self-stretch"
      data-tauri-drag-region
    >
      <WorkspacePicker v-if="!isHome" icon-only :drop-down="true" />
      <span
        v-if="chat.running"
        class="mx-0.5 h-1 w-1 shrink-0 rounded-full bg-ok lw-pulse"
        :title="t('status.running')"
      />
      <span
        class="title-text min-w-0 truncate text-[14px] font-medium tracking-[-0.02em] text-ink-1"
        :title="title"
        data-tauri-drag-region
        >{{ title }}</span
      >
    </div>

    <!-- 动作组（no-drag）：v3.9 只剩面板开关（新建/搜索移除，issue #3）；
         首页空态下隐藏（issue #10：空白页不可展开右栏）。v2.60 修复批：
         去掉 wash 底色容器（开关裸钮），且面板开合不移位——车道恒预留 -->
    <div v-if="!isHome" class="flex shrink-0 items-center">
      <button
        class="ctl"
        type="button"
        :aria-pressed="workPanel.open"
        :title="t('panel.title')"
        :aria-label="t('panel.title')"
        @click="workPanel.toggle()"
      >
        <Icon name="panel" :size="14" aria-hidden="true" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.ctl {
  display: inline-flex;
  width: var(--lw-ctl);
  height: var(--lw-ctl);
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-md);
  color: var(--lw-ink-2);
}
.ctl:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}

/* 标题固定宽度上限（v2.60 修复批）：长标题在此截断省略，不再横贯整条带
 * （也保证面板开合时截断宽度稳定不跳） */
.title-text {
  max-width: 360px;
}

/* lead 槽：宽 0 → 28px；展开侧栏时用 --dur-base + ease-out（与侧栏同步），
 * 收起回到 0 时 --dur-fast + ease-in（§2.2-6 的"标题零跳变"靠同速对齐） */
.lead {
  width: 0;
  align-self: stretch;
  transition: width var(--dur-fast) var(--ease-in);
}
.lead--open {
  width: calc(var(--lw-ctl) + 4px);
  transition: width var(--dur-base) var(--ease-out);
}
</style>
