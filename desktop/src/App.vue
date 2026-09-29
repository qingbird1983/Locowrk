<script setup lang="ts">
import { onMounted } from "vue";
import { useI18n } from "vue-i18n";
import { useApprovalsStore } from "./stores/approvals";
import { useChatStore } from "./stores/chat";
import ApprovalModal from "./components/ApprovalModal.vue";
import ChatMessages from "./components/ChatMessages.vue";
import PromptBar from "./components/PromptBar.vue";

const { t } = useI18n();
const chat = useChatStore();
const approvals = useApprovalsStore();
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
    </header>
    <ChatMessages />
    <PromptBar />
    <ApprovalModal />
  </div>
</template>
