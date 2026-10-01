<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../stores/chat";

const { t } = useI18n();
const chat = useChatStore();
const draft = ref("");

function send() {
  const text = draft.value;
  if (!text.trim() || chat.running) return;
  draft.value = "";
  chat.send(text);
}
</script>

<template>
  <footer class="border-t border-line px-5 py-3">
    <div class="mx-auto flex max-w-3xl items-end gap-2">
      <textarea
        v-model="draft"
        rows="1"
        :placeholder="t('chat.placeholder')"
        class="flex-1 resize-none rounded-xl border border-line-strong bg-surface-1 px-3 py-2 text-sm outline-none placeholder:text-ink-5 focus:border-emerald-600"
        @keydown.enter.exact.prevent="send"
      />
      <button
        class="rounded-xl bg-emerald-600 px-4 py-2 text-sm font-medium text-white transition hover:bg-emerald-500 disabled:cursor-not-allowed disabled:opacity-40"
        :disabled="chat.running || !draft.trim()"
        @click="send"
      >
        {{ t("chat.send") }}
      </button>
    </div>
  </footer>
</template>
