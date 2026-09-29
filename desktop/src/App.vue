<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useApprovalsStore } from "./stores/approvals";
import { useChatStore } from "./stores/chat";
import ApprovalModal from "./components/ApprovalModal.vue";
import ChatMessages from "./components/ChatMessages.vue";
import PluginMarket from "./components/PluginMarket.vue";
import PromptBar from "./components/PromptBar.vue";

const { t } = useI18n();
const chat = useChatStore();
const approvals = useApprovalsStore();
const view = ref<"chat" | "plugins">("chat");
onMounted(() => {
  chat.init();
  approvals.init();
});
</script>

<template>
  <div class="flex h-full flex-col bg-neutral-900 text-neutral-100">
    <header
      class="flex items-center justify-between border-b border-neutral-800 px-5 py-3"
    >
      <div>
        <h1 class="text-sm font-semibold tracking-wide">Locowrk</h1>
        <p class="text-xs text-neutral-500">{{ t("app.subtitle") }}</p>
      </div>
      <div class="flex items-center gap-2">
        <!-- 顶级导航（§13.1）：聊天 / 插件市场 -->
        <nav class="flex gap-1 rounded-lg bg-neutral-800/70 p-1">
          <button
            v-for="key in (['chat', 'plugins'] as const)"
            :key="key"
            class="rounded-md px-3 py-1 text-xs font-medium transition"
            :class="
              view === key
                ? 'bg-neutral-700 text-neutral-100'
                : 'text-neutral-500 hover:text-neutral-300'
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
              ? 'bg-amber-900/60 text-amber-300'
              : 'bg-neutral-800 text-neutral-400'
          "
        >
          {{ chat.running ? t("status.running") : t("status.ready") }}
        </span>
      </div>
    </header>
    <!-- 聊天视图用 v-show 保状态；插件页按需挂载 -->
    <div v-show="view === 'chat'" class="flex min-h-0 flex-1 flex-col">
      <ChatMessages />
      <PromptBar />
    </div>
    <PluginMarket v-if="view === 'plugins'" />
    <ApprovalModal />
  </div>
</template>
