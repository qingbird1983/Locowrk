<script setup lang="ts">
/**
 * 会话顶栏（UI shell doc §2.1 ③ / §2.2）：46px 带中段。折叠按钮的 lead
 * 槽恒挂（展开时宽 0——零宽导致标题零跳变，§2.2-6）；标题段是拖拽区；
 * 右端窗口控制 120px 车道按 §2.2-5 单一负责。v3.9（issue #3⑩）：
 * 新建/搜索钮移除（侧栏顶部动作区已有，重复）；工作面板开关首页空态下
 * 隐藏、不可展开（§2.4）。v2.60 修复批：①项目归属钮移进顶栏——标题前
 * 纯图标（WorkspacePicker iconOnly，菜单下弹），composer 底行只剩首页
 * 形态；②标题固定宽度上限，放不下尾部省略。v2.61 用户裁定「展开右栏
 * 还是被挤跑」：面板开关改 **fixed 钉窗**（right = 车道 120px 恒定）——
 * 文档流里无论怎么预留，主区本身随面板开合收缩都会被挤动；面板头为其
 * 预留槽位（原「关闭面板」钮位置），开合零位移。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { useShellStore } from "../../stores/shell";
import { useWorkPanelStore } from "../../stores/work-panel";
import { Icon } from "../../lib/icons";
import HoverTip from "../settings/HoverTip.vue";
import WorkspacePicker from "../chat/WorkspacePicker.vue";
import SubagentsMenu from "./SubagentsMenu.vue";

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
    style="padding-right: calc(var(--lw-wincontrols) + 40px)"
    data-tauri-drag-region
  >
    <!-- lead 槽：恒在，展开时宽 0（过渡与侧栏同步，标题不跳变）。
         §3.40④（v2.111）：左栏切换钮任何宽度可用——合 = 这里的展开钮，
         开 = 侧栏头内收起钮，任何时刻恰有一个可见可点 -->
    <div
      class="lead flex items-center overflow-hidden"
      :class="shell.sidebarCollapsed ? 'lead--open' : ''"
      :aria-hidden="!shell.sidebarCollapsed"
      :inert="!shell.sidebarCollapsed"
    >
      <HoverTip :text="t('sidebar.expand')">
        <button
          class="ctl"
          type="button"
          :aria-label="t('sidebar.expand')"
          :tabindex="shell.sidebarCollapsed ? 0 : -1"
          @click="shell.toggleSidebar()"
        >
          <Icon name="sidebar" :size="14" aria-hidden="true" />
        </button>
      </HoverTip>
    </div>

    <!-- 标题段（拖拽区）：项目归属图标（v2.60，会话态）→ 运行点 → 标题。
         标题固定宽度上限：放不下尾部省略（v2.60 修复批） -->
    <div
      class="flex min-w-0 flex-1 items-center gap-1.5 self-stretch"
      data-tauri-drag-region
    >
      <WorkspacePicker v-if="!isHome" icon-only :drop-down="true" />
      <HoverTip
        v-if="chat.running"
        class="shrink-0"
        :text="t('status.running')"
        :label="t('status.running')"
      >
        <span class="mx-0.5 h-1 w-1 shrink-0 rounded-full bg-ok lw-pulse" />
      </HoverTip>
      <HoverTip class="min-w-0" :text="title" :label="title">
        <span
          class="title-text min-w-0 truncate text-[length:var(--lw-fs-14)] font-medium tracking-[-0.02em] text-ink-1"
          data-tauri-drag-region
          >{{ title }}</span
        >
      </HoverTip>
      <!-- 子代理入口（§2.1 ③ v4.18）：标题后；无 subagent_run 活动整件不渲染 -->
      <SubagentsMenu />
    </div>

  </header>
  <!-- 右上角面板开关钉窗（§3.40④；v2.61 钉窗语义）。fixed 定位挂在全局类
       .lw-topbar-toggles（style.css）——scoped 属性进不了多根组件内部
       （v2.85/v2.61 先例）。面板钮首页空态隐藏（§2.4 页面语义，非宽度
       限制）；任何 ≥400px 宽度可点（v2.111：<1400 禁用态退役——断点只
       决定展开模式，overlay 档自动互斥收左栏）。z-30 压过工作面板
       z-20/浮层 z-26，header/面板头让位车道 +40px。左栏不设顶栏开关
       （v2.110 目检裁定） -->
  <div class="lw-topbar-toggles">
    <HoverTip v-if="!isHome" :text="t('panel.title')">
      <button
        class="ctl"
        type="button"
        :aria-pressed="workPanel.open"
        :aria-label="t('panel.title')"
        @click="workPanel.toggle()"
      >
        <Icon name="panel" :size="14" aria-hidden="true" />
      </button>
    </HoverTip>
  </div>
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

/* 钉窗定位已上收为全局类 .lw-topbar-toggles（style.css，§3.40④）——
 * scoped 属性进不了多根组件（HoverTip）内部，规则必须放全局才能落到包裹层 */

/* 标题固定宽度上限（v2.60 修复批）：长标题在此截断省略，不再横贯整条带 */
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
