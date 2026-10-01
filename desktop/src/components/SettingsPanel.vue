<script setup lang="ts">
/**
 * 设置面板（壳批次后 = 分区容器，docs/design/UI shell and patterns.md
 * §3.2）：settings.json 分区 ↔ 面板分区一一对应——llm / embedding（⑬）/
 * context（⑨）/ memory（⑪ + ⑬ autoExtract）/ tools（⑫ 开发者选项，含
 * 暴露调试面板）/ shortcut / locale（语言）/ appearance（⑰ 主题）/
 * plugins（⑬ 自动停用）为实装区；sessions 只读展示。各分区自管表单与
 * 保存动作，本文件只做纵向堆叠与错误条，不写业务。
 */
import { useI18n } from "vue-i18n";
import { useSettingsStore } from "../stores/settings";
import AppearanceSection from "./settings/AppearanceSection.vue";
import ContextSection from "./settings/ContextSection.vue";
import DeveloperSection from "./settings/DeveloperSection.vue";
import EmbeddingSection from "./settings/EmbeddingSection.vue";
import LanguageSection from "./settings/LanguageSection.vue";
import LlmSection from "./settings/LlmSection.vue";
import MemorySection from "./settings/MemorySection.vue";
import PluginsSection from "./settings/PluginsSection.vue";
import SessionsSection from "./settings/SessionsSection.vue";
import ShellSection from "./settings/ShellSection.vue";
import ShortcutSection from "./settings/ShortcutSection.vue";

const { t } = useI18n();
const settings = useSettingsStore();
</script>

<template>
  <div class="mx-auto w-full max-w-2xl space-y-4 overflow-y-auto p-4">
    <h2 class="text-sm font-semibold">{{ t("settings.title") }}</h2>

    <p
      v-if="settings.error"
      class="rounded-lg bg-danger/10 px-3 py-2 text-xs text-danger"
    >
      {{ settings.error }}
    </p>

    <LlmSection />

    <!-- ⑬ embedding 模型：工具语义检索 / 经历记忆 / 隐式提取的共享基建 -->
    <EmbeddingSection />

    <!-- ⑨ 上下文管理：窗口 / 阈值滑条 / 策略三档（占位已由真分区替换） -->
    <ContextSection />

    <!-- ⑪ 记忆分区：注入开关 + 注入条数 + ⑬ 隐式提取开关 -->
    <MemorySection />

    <SessionsSection />
    <LanguageSection />
    <!-- ⑰ 外观：明暗主题三档（保存即生效） -->
    <AppearanceSection />
    <ShortcutSection />

    <!-- S1 命令终端治理：高危命令解锁开关（重启生效；硬锁类不受影响） -->
    <ShellSection />

    <!-- ⑬ 插件管理：闲置自动停用（缺省关） -->
    <PluginsSection />

    <!-- ⑫ 开发者分区：工具暴露截断设置 + 最近一次暴露分布 / token 节省估算 -->
    <DeveloperSection />
  </div>
</template>
