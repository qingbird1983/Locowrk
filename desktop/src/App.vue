<script setup lang="ts">
import { onMounted } from "vue";
import { useChatStore } from "./stores/chat";
import ChatMessages from "./components/ChatMessages.vue";
import PromptBar from "./components/PromptBar.vue";

const chat = useChatStore();
onMounted(() => chat.init());
</script>

<template>
  <div class="flex h-full flex-col bg-neutral-900 text-neutral-100">
    <header
      class="flex items-center justify-between border-b border-neutral-800 px-5 py-3"
    >
      <div>
        <h1 class="text-sm font-semibold tracking-wide">Locowrk</h1>
        <p class="text-xs text-neutral-500">
          阶段五脚手架 · 模型 = 脚本替身 · 工具 = tool-echo 插件
        </p>
      </div>
      <span
        class="rounded-full px-2.5 py-1 text-xs"
        :class="chat.running ? 'bg-amber-900/60 text-amber-300' : 'bg-neutral-800 text-neutral-400'"
      >
        {{ chat.running ? "运行中…" : "就绪" }}
      </span>
    </header>
    <ChatMessages />
    <PromptBar />
  </div>
</template>
