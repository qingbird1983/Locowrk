<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useApprovalsStore } from "./stores/approvals";
import { useChatStore } from "./stores/chat";
import { useSettingsStore } from "./stores/settings";
import ApprovalModal from "./components/ApprovalModal.vue";
import ChatMessages from "./components/ChatMessages.vue";
import ContextProgressBar from "./components/ContextProgressBar.vue";
import MemoryPage from "./components/MemoryPage.vue";
import PluginMarket from "./components/PluginMarket.vue";
import PromptBar from "./components/PromptBar.vue";
import SessionSwitcher from "./components/chat/SessionSwitcher.vue";
import SettingsPanel from "./components/SettingsPanel.vue";

const { t } = useI18n();
const chat = useChatStore();
const approvals = useApprovalsStore();
const settings = useSettingsStore();
/** 顶层导航定稿（壳批次）：4 视图；>4 时 pill nav 切侧边栏（UI shell
 * and patterns.md §2）。 */
const view = ref<"chat" | "plugins" | "memory" | "settings">("chat");
onMounted(() => {
  chat.init();
  approvals.init();
  void settings.init();
});
</script>

<template>
  <div class="flex h-full flex-col bg-surface-0 text-ink-1">
    <header
      class="flex items-center justify-between border-b border-line px-5 py-3"
    >
      <div>
        <h1 class="text-sm font-semibold tracking-wide">Locowrk</h1>
        <!-- 副标题实时反映当前模型（设置改动即生效，§13.1） -->
        <p class="text-xs text-ink-4">
          {{
            t("app.subtitle", {
              model:
                settings.view?.llm.model.trim() ||
                t("app.modelFallback"),
            })
          }}
        </p>
      </div>
      <div class="flex items-center gap-2">
        <!-- 顶级导航（壳批次定稿）：对话 / 插件 / 记忆 / 设置 -->
        <nav class="flex gap-1 rounded-lg bg-wash-2 p-1">
          <button
            v-for="key in (['chat', 'plugins', 'memory', 'settings'] as const)"
            :key="key"
            class="rounded-md px-3 py-1 text-xs font-medium transition"
            :class="
              view === key
                ? 'bg-wash-3 text-ink-1'
                : 'text-ink-4 hover:text-ink-2'
            "
            @click="view = key"
          >
            {{ t(`nav.${key}`) }}
          </button>
        </nav>
        <span
          class="rounded-full px-2.5 py-1 text-xs"
          :class="
            chat.running
              ? 'bg-warn/16 text-warn'
              : 'bg-wash-2 text-ink-3'
          "
        >
          {{ chat.running ? t("status.running") : t("status.ready") }}
        </span>
      </div>
    </header>
    <!-- 聊天视图用 v-show 保状态；其余视图按需挂载 -->
    <div v-show="view === 'chat'" class="flex min-h-0 flex-1 flex-col">
      <ContextProgressBar :usage="chat.contextUsage" />
      <SessionSwitcher />
      <ChatMessages />
      <PromptBar />
    </div>
    <PluginMarket v-if="view === 'plugins'" />
    <MemoryPage v-if="view === 'memory'" />
    <SettingsPanel v-if="view === 'settings'" />
    <ApprovalModal />
  </div>
</template>
