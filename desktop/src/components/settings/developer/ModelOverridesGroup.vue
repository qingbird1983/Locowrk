<script setup lang="ts">
/**
 * 设置分区子块：模型参数覆盖（S4 刀④ 自服务商弹窗高级区迁入）。
 * 缺省跟随 models.dev 目录；这里编辑 = 钉死。普通分区只保留只读徽章。
 */
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { EMPTY_SOURCES, persistableRows } from "../../../lib/model-catalog";
import { useSettingsStore } from "../../../stores/settings";
import type { ProviderModelView, ProviderView } from "../../../types";
import SettingsGroup from "../SettingsGroup.vue";
import SettingsRow from "../SettingsRow.vue";

const { t } = useI18n();
const settings = useSettingsStore();

const llmProviders = computed<ProviderView[]>(
  () => settings.view?.llm.providers ?? [],
);

/** 编辑即钉死：把对应来源标成 user，否则 persistableRows 会把它当目录派生值剥掉。 */
function pinned(row: ProviderModelView, patch: Partial<ProviderModelView>): ProviderModelView {
  const sources = { ...(row.sources ?? EMPTY_SOURCES) };
  if ("contextWindow" in patch) sources.window = "user";
  if ("maxOutput" in patch) sources.maxOutput = "user";
  return { ...row, ...patch, sources };
}

async function saveModelParam(
  p: ProviderView,
  modelId: string,
  patch: Partial<ProviderModelView>,
) {
  const next = p.models.map((m) => (m.id === modelId ? pinned(m, patch) : m));
  const persistable = persistableRows(next);
  // 首项 = 该档默认模型；窗口随它走（同服务商弹窗保存口径）。改其余模型
  // 不动活动窗口，避免一次覆盖把用户既有窗口设置重置。
  const first = persistable[0];
  await settings.saveProvider({
    id: p.id,
    label: p.label,
    kind: p.kind,
    baseUrl: p.baseUrl,
    model: p.model,
    windowTokens:
      modelId === first?.id ? (first.contextWindow ?? null) : p.windowTokens,
    enabled: p.enabled,
    models: persistable,
    headers: p.headers,
  });
}
</script>

<template>
  <SettingsGroup
    :title="t('developer.modelOverridesTitle')"
    :hint="t('developer.modelOverridesHint')"
  >
    <p
      v-if="llmProviders.length === 0"
      class="px-1 py-1 text-[length:var(--lw-fs-12)] text-ink-4"
    >
      {{ t("developer.modelOverridesEmpty") }}
    </p>
    <template v-for="p in llmProviders" :key="p.id">
      <div
        v-if="p.models.length"
        class="px-1 pt-2 text-[length:var(--lw-fs-11)] font-medium text-ink-4"
      >
        {{ p.label }}
      </div>
      <SettingsRow v-for="m in p.models" :key="`${p.id}:${m.id}`" :title="m.id">
        <div class="flex items-center gap-2">
          <span class="shrink-0 whitespace-nowrap text-[length:var(--lw-fs-11)] text-ink-4">
            {{ t("developer.modelWindowShort") }}
          </span>
          <input
            type="number"
            min="1"
            inputmode="numeric"
            :value="m.contextWindow ?? ''"
            class="lw-field lw-field-num w-24 px-2 text-xs"
            :aria-label="`${p.label} ${m.id} ${t('developer.modelWindowShort')}`"
            @change="
              saveModelParam(p, m.id, {
                contextWindow: Number(($event.target as HTMLInputElement).value) || null,
              })
            "
          />
          <span class="shrink-0 whitespace-nowrap text-[length:var(--lw-fs-11)] text-ink-4">
            {{ t('developer.modelMaxOutputShort') }}
          </span>
          <input
            type="number"
            min="1"
            inputmode="numeric"
            :value="m.maxOutput ?? ''"
            class="lw-field lw-field-num w-24 px-2 text-xs"
            :aria-label="`${p.label} ${m.id} ${t('developer.modelMaxOutputShort')}`"
            @change="
              saveModelParam(p, m.id, {
                maxOutput: Number(($event.target as HTMLInputElement).value) || null,
              })
            "
          />
        </div>
      </SettingsRow>
    </template>
  </SettingsGroup>
</template>
