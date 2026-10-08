<script setup lang="ts">
/**
 * 导入面板（UI shell v4.26 批 F）：从 claude-code / codex / opencode /
 * cc-switch 导入 AI 服务配置。打开即扫描（llm_import_scan 纯读）；候选
 * 行 = 勾选 + 来源徽章 + 名称 + 端点 + N 个模型 + Key 态 + note 灰字；
 * 「导入选中」= llm_import_apply（新档追加，逐条结果 toast）。密钥不出
 * 后端（hasKey 布尔）。Esc / 取消 / overlay 点击关。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { llmImportApply, llmImportScan } from "../../../lib/api";
import { Icon } from "../../../lib/icons";
import { useToastStore } from "../../../stores/toast";
import type { ImportCandidate, ImportScanOutcome } from "../../../types";

const emit = defineEmits<{ close: []; imported: [] }>();
const { t } = useI18n();
const toast = useToastStore();

const scanning = ref(true);
const scanError = ref("");
const outcome = ref<ImportScanOutcome | null>(null);
const checked = ref<Set<string>>(new Set());
const applying = ref(false);

const candidates = computed(() => outcome.value?.candidates ?? []);
const errors = computed(() => outcome.value?.errors ?? []);
const checkedCount = computed(() => checked.value.size);

async function scan(): Promise<void> {
  scanning.value = true;
  scanError.value = "";
  try {
    outcome.value = await llmImportScan();
    // 缺省全选有 Key 的候选（用户取消勾选即减项）
    checked.value = new Set(
      outcome.value.candidates.filter((c) => c.hasKey).map((c) => c.id),
    );
  } catch (e) {
    scanError.value = String(e);
    outcome.value = null;
  } finally {
    scanning.value = false;
  }
}

function toggle(id: string): void {
  const next = new Set(checked.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  checked.value = next;
}

const SOURCE_LABELS: Record<ImportCandidate["source"], string> = {
  "claude-code": "claude code",
  codex: "codex",
  opencode: "opencode",
  "cc-switch": "cc-switch",
};

async function applySelected(): Promise<void> {
  const ids = [...checked.value];
  if (ids.length === 0) return;
  applying.value = true;
  try {
    const report = await llmImportApply(ids);
    if (report.imported.length > 0) {
      toast.push("ok", t("settings.importAppliedToast", { n: report.imported.length }));
      emit("imported");
      emit("close");
    } else if (report.skipped.length > 0) {
      toast.push("danger", report.skipped.join("；"));
    }
  } catch (e) {
    toast.push("danger", String(e));
  } finally {
    applying.value = false;
  }
}

function onKey(e: KeyboardEvent): void {
  if (e.key === "Escape") emit("close");
}

onMounted(() => {
  document.addEventListener("keydown", onKey);
  void scan();
});
onBeforeUnmount(() => document.removeEventListener("keydown", onKey));
</script>

<template>
  <teleport to="body">
    <div
      class="fixed inset-0 z-[70] flex items-center justify-center bg-black/40"
      @pointerdown.self="emit('close')"
    >
      <div
        class="flex max-h-[80vh] w-[min(560px,calc(100vw-48px))] flex-col rounded-[var(--lw-r-lg)] bg-[var(--lw-bg-elevated)] shadow-[var(--lw-shadow-raised)]"
        role="dialog"
        :aria-label="t('settings.importTitle')"
      >
        <div class="flex items-center gap-2 border-b border-[var(--lw-line-subtle)] px-5 py-3.5">
          <h3 class="text-[length:var(--lw-fs-14)] font-semibold text-ink-1">
            {{ t("settings.importTitle") }}
          </h3>
          <button
            type="button"
            class="ml-auto rounded-[var(--lw-r-xs)] p-1 text-ink-3 transition hover:bg-wash-2 hover:text-ink-1"
            :aria-label="t('common.close')"
            @click="emit('close')"
          >
            <Icon name="close" :size="16" />
          </button>
        </div>

        <div class="min-h-0 flex-1 overflow-y-auto px-5 py-3">
          <p class="mb-3 text-[length:var(--lw-fs-12)] leading-relaxed text-ink-3">
            {{ t("settings.importHint") }}
          </p>

          <p v-if="scanning" class="py-6 text-center text-[length:var(--lw-fs-13)] text-ink-3">
            {{ t("settings.importScanning") }}
          </p>
          <p v-else-if="scanError" class="py-6 text-center text-[length:var(--lw-fs-13)] text-danger">
            {{ scanError }}
          </p>
          <template v-else>
            <p
              v-if="candidates.length === 0"
              class="py-6 text-center text-[length:var(--lw-fs-13)] text-ink-3"
            >
              {{ t("settings.importEmpty") }}
            </p>
            <ul v-else class="space-y-1.5">
              <li
                v-for="c in candidates"
                :key="c.id"
                class="rounded-[var(--lw-r-md)] bg-wash-1 px-3 py-2.5"
              >
                <label class="flex cursor-pointer items-start gap-2.5">
                  <input
                    type="checkbox"
                    class="mt-0.5 h-3.5 w-3.5 shrink-0 accent-[var(--lw-accent)]"
                    :checked="checked.has(c.id)"
                    @change="toggle(c.id)"
                  />
                  <span class="min-w-0 flex-1">
                    <span class="flex items-center gap-2">
                      <span
                        class="shrink-0 rounded-full bg-wash-3 px-1.5 py-0.5 font-mono text-[length:var(--lw-fs-11)] text-ink-3"
                      >{{ SOURCE_LABELS[c.source] }}</span>
                      <span class="min-w-0 truncate text-[length:var(--lw-fs-13)] font-medium text-ink-1">{{ c.label }}</span>
                      <span
                        class="ml-auto shrink-0 text-[length:var(--lw-fs-11)]"
                        :class="c.hasKey ? 'text-ok' : 'text-warn'"
                      >{{ c.hasKey ? t("settings.importHasKey") : t("settings.importNoKey") }}</span>
                    </span>
                    <span class="mt-0.5 flex flex-wrap items-center gap-x-2 text-[length:var(--lw-fs-11)] text-ink-4">
                      <span class="truncate font-mono">{{ c.baseUrl ?? "—" }}</span>
                      <span>·</span>
                      <span>{{ t("settings.importModelsCount", { n: c.models.length }) }}</span>
                      <span v-if="c.model">·</span>
                      <span v-if="c.model" class="truncate font-mono">{{ c.model }}</span>
                    </span>
                    <span
                      v-for="note in c.notes"
                      :key="note"
                      class="mt-0.5 block text-[length:var(--lw-fs-11)] leading-relaxed text-warn"
                    >{{ note }}</span>
                  </span>
                </label>
              </li>
            </ul>
            <ul v-if="errors.length > 0" class="mt-3 space-y-0.5">
              <li
                v-for="e in errors"
                :key="e"
                class="text-[length:var(--lw-fs-11)] leading-relaxed text-ink-5"
              >{{ e }}</li>
            </ul>
          </template>
        </div>

        <div class="flex items-center gap-2 border-t border-[var(--lw-line-subtle)] px-5 py-3">
          <button
            type="button"
            class="lw-btn px-3 py-1.5 text-xs text-ink-1"
            @click="scan"
            :disabled="scanning"
          >
            <Icon name="refresh" :size="14" />
            {{ t("settings.importRescan") }}
          </button>
          <button
            type="button"
            class="ml-auto rounded-[var(--lw-r-xs)] bg-accent px-3.5 py-1.5 text-xs font-semibold text-accent-ink transition hover:opacity-90 disabled:opacity-50"
            :disabled="checkedCount === 0 || applying"
            @click="applySelected"
          >
            {{ applying ? t("settings.importApplying") : t("settings.importApply", { n: checkedCount }) }}
          </button>
        </div>
      </div>
    </div>
  </teleport>
</template>
