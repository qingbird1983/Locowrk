<script setup lang="ts">
import { onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useApprovalsStore } from "./stores/approvals";
import { useChatStore } from "./stores/chat";
import { useNotificationsStore } from "./stores/notifications";
import { useSettingsStore } from "./stores/settings";
import { useShellStore } from "./stores/shell";
import ChatTranscript from "./components/ChatTranscript.vue";
import Composer from "./components/chat/Composer.vue";
import ContextProgressBar from "./components/ContextProgressBar.vue";
import MemoryPage from "./components/MemoryPage.vue";
import PluginMarket from "./components/PluginMarket.vue";
import SettingsPage from "./components/settings/SettingsPage.vue";
import CommandPalette from "./components/shell/CommandPalette.vue";
import DestinationShell from "./components/shell/DestinationShell.vue";
import Sidebar from "./components/shell/Sidebar.vue";
import ToastStack from "./components/shell/ToastStack.vue";
import Topbar from "./components/shell/Topbar.vue";
import WindowControls from "./components/shell/WindowControls.vue";
import { ref } from "vue";

/**
 * 壳骨架（UI shell doc §2.1 区域地图 / §2.6 保活规则）：
 * - 目的地一律整页接管 ③④⑤；设置更极端——连侧栏一起接管（§3.32，
 *   导航轨 + 页头 + 正文列自成一壳）；
 * - 对话视图不卸载：切走时 display:none + inert 挂起，回来恢复滚动与
 *   运行态（hidden 会被 flex 类覆盖，故用内联 display）；
 * - 其他目的地按需挂载（v-if），离开即卸载；侧栏（非设置目的地）恒显；
 * - 全局层：⌘K 命令面板 / 窗口控制挂壳根（跨视图）。审批是转录内的
 *   内联决策卡（§3.17，㉕·对话区① 起 ApprovalModal 退役）。
 */
const { t } = useI18n();
const chat = useChatStore();
const approvals = useApprovalsStore();
const settings = useSettingsStore();
const shell = useShellStore();
const notifications = useNotificationsStore();
/** composer 坞总高：列宽热区下沿（ConversationWidthHandles）的锚。 */
const composerH = ref(0);

onMounted(() => {
  chat.init();
  approvals.init();
  void settings.init();
  void notifications.init();
});
</script>

<template>
  <div class="isolate flex h-full bg-page text-ink-1">
    <!-- Toast 栈（§3.22）：全局一次挂载，右下角 -->
    <ToastStack />
    <!-- 设置目的地：独立壳模式，整窗接管（§3.32 导航轨 + 页头 + 正文列） -->
    <template v-if="shell.destination === 'settings'">
      <SettingsPage />
    </template>

    <template v-else>
      <Sidebar />

      <!-- 对话主区：目的地切换时挂起，回来即恢复滚动与在跑的会话。
           data-chat-root = 列宽覆盖变量与拖拽锁的挂点（§2.7） -->
      <div
        data-chat-root
        class="relative flex min-w-0 flex-1 flex-col"
        :style="{ display: shell.destination === 'chat' ? '' : 'none' }"
        :inert="shell.destination !== 'chat'"
      >
        <Topbar />
        <ContextProgressBar :usage="chat.contextUsage" />
        <ChatTranscript :composer-bottom="composerH" />
        <Composer @height="composerH = $event" />
      </div>

      <!-- 非对话目的地：侧栏右侧铺满，按需挂载 -->
      <DestinationShell
        v-if="shell.destination === 'plugins'"
        :title="t('nav.plugins')"
      >
        <PluginMarket />
      </DestinationShell>
      <DestinationShell
        v-else-if="shell.destination === 'memory'"
        :title="t('nav.memory')"
      >
        <MemoryPage />
      </DestinationShell>
    </template>

    <WindowControls />
    <CommandPalette />
  </div>
</template>
