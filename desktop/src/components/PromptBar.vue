<script setup lang="ts">
import { ref } from "vue";
import { useChatStore } from "../stores/chat";

const chat = useChatStore();
const draft = ref("");

function send() {
  const t = draft.value;
  if (!t.trim() || chat.running) return;
  draft.value = "";
  chat.send(t);
}
</script>

<template>
  <footer class="border-t border-neutral-800 px-5 py-3">
    <div class="mx-auto flex max-w-3xl items-end gap-2">
      <textarea
        v-model="draft"
        rows="1"
        placeholder="输入消息…（Enter 发送，Shift+Enter 换行）"
        class="flex-1 resize-none rounded-xl border border-neutral-700 bg-neutral-800 px-3 py-2 text-sm outline-none placeholder:text-neutral-600 focus:border-emerald-600"
        @keydown.enter.exact.prevent="send"
      />
      <button
        class="rounded-xl bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="chat.running || !draft.trim()"
        @click="send"
      >
        发送
      </button>
    </div>
  </footer>
</template>
