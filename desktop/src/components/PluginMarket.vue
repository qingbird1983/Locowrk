<script setup lang="ts">
/**
 * 插件页容器（壳批次拆分后）：页签 + 错误条，内容在两个子页签组件里
 * （发现 = 卡片网格，已安装 = 列表页模式）。数据与动作全在 plugins
 * store（§13.3/§13.4），本文件不写业务。
 */
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { usePluginStore } from "../stores/plugins";
import DiscoverTab from "./plugins/DiscoverTab.vue";
import InstalledTab from "./plugins/InstalledTab.vue";

const { t } = useI18n();
const store = usePluginStore();
const tab = ref<"discover" | "installed">("discover");

onMounted(() => {
  void store.init();
});
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col overflow-hidden">
    <!-- 页签 -->
    <div
      class="flex items-center gap-1 border-b border-neutral-800 px-5 py-2"
    >
      <button
        v-for="key in (['discover', 'installed'] as const)"
        :key="key"
        class="rounded-lg px-3 py-1.5 text-xs font-medium transition"
        :class="
          tab === key
            ? 'bg-neutral-700 text-neutral-100'
            : 'text-neutral-500 hover:text-neutral-300'
        "
        @click="tab = key"
      >
        {{ t(`market.${key}`) }}
      </button>
    </div>

    <!-- 错误条 -->
    <p
      v-if="store.error"
      class="border-b border-red-900/50 bg-red-950/50 px-5 py-1.5 text-xs text-red-300"
    >
      {{ store.error }}
    </p>

    <DiscoverTab v-if="tab === 'discover'" />
    <InstalledTab v-else />
  </div>
</template>
