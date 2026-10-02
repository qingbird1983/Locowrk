<script setup lang="ts">
/**
 * 会话顶栏（UI shell doc §2.1 ③ / §2.2）：46px 带中段。折叠按钮的 lead
 * 槽恒挂（展开时宽 0——零宽导致标题零跳变，§2.2-6）；标题段是拖拽区；
 * 右端窗口控制 120px 车道按 §2.2-5 单一负责：面板关闭时顶栏预留，面板
 * 打开时让位（padding-right:0，车道由面板头接管）。动作组 = 原型
 * .action-group 同款 wash-1 圆角容器：新建+搜索一组、面板开关独立一组。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { useShellStore } from "../../stores/shell";
import { useWorkPanelStore } from "../../stores/work-panel";
import { Icon } from "../../lib/icons";

const { t } = useI18n();
const chat = useChatStore();
const shell = useShellStore();
const workPanel = useWorkPanelStore();

const title = computed(() => {
  // 首页空态（㉕·对话区①）：无会话打开 = 标题空（§3.14 第一屏）
  if (chat.items.length === 0) return "";
  return (
    chat.sessions.find((s) => s.id === chat.sessionId)?.title ??
    t("sidebar.untitled")
  );
});

function focusSidebarSearch() {
  shell.setDestination("chat");
  // 侧栏折叠时先展开，再聚焦其搜索框
  if (shell.sidebarCollapsed) shell.toggleSidebar();
  requestAnimationFrame(() => {
    document
      .querySelector<HTMLInputElement>('.sidebar input[type="text"]')
      ?.focus();
  });
}
</script>

<template>
  <header
    class="relative z-10 flex h-[var(--lw-toolbar-h)] shrink-0 items-center bg-page pl-3 transition-[padding-right] duration-150"
    :style="{ paddingRight: workPanel.open ? '0' : 'var(--lw-wincontrols)' }"
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

    <!-- 标题段（拖拽区）：运行中挂脉动点（§6.6 ⑧） -->
    <div
      class="flex min-w-0 flex-1 items-center gap-2 self-stretch"
      data-tauri-drag-region
    >
      <span
        v-if="chat.running"
        class="h-1 w-1 shrink-0 rounded-full bg-ok lw-pulse"
        :title="t('status.running')"
      />
      <span
        class="min-w-0 truncate text-[14px] font-medium tracking-[-0.02em] text-ink-1"
        :title="title"
        data-tauri-drag-region
        >{{ title }}</span
      >
    </div>

    <!-- 动作组（no-drag）：新建 / 搜索一组，面板开关独立一组（§2.4） -->
    <div class="action-group shrink-0">
      <button
        class="ctl"
        type="button"
        :title="t('sidebar.newChat')"
        :aria-label="t('sidebar.newChat')"
        @click="chat.newSession()"
      >
        <Icon name="plus" aria-hidden="true" />
      </button>
      <button
        class="ctl"
        type="button"
        :title="t('sidebar.search')"
        :aria-label="t('sidebar.search')"
        @click="focusSidebarSearch"
      >
        <Icon name="search" aria-hidden="true" />
      </button>
    </div>
    <div class="action-group ml-1.5 shrink-0">
      <!-- 工作面板开关（§2.4/§2.5）：独立成组，只对对话目的地有意义 -->
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

/* 顶层动作组（原型 .action-group）：28px 图标按钮装在 12px 圆角 wash 容器 */
.action-group {
  display: flex;
  align-items: center;
  gap: 2px;
  padding: 2px;
  border-radius: var(--lw-r-md);
  background: var(--lw-wash-1);
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
