<script setup lang="ts">
/**
 * 默认模型更改浮层（UI shell 文档 §3.30 默认项行）：锚触发钮下方右对齐，
 * 320px 搜索 + 按服务分组（组标签 11px ink-4 不可点；项 = ✓ + 模型名
 * mono，hover wash-2），打开滚到当前项；无结果显示空态文案。数据 =
 * 已配置清单（启用档 × 其 models，档无 models 时回落其单 model）——
 * 与 PI 同口径：默认选择器列「已配置」，不现场探测。纯受控组件。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../../lib/icons";
import type { ProviderView } from "../../../types";

const props = defineProps<{
  providers: ProviderView[];
  /** 当前生效（activeId, model）。 */
  activeId: string;
  activeModel: string;
  anchor: HTMLElement | null;
}>();
const emit = defineEmits<{
  select: [providerId: string, model: string];
  close: [];
}>();

const { t } = useI18n();
const query = ref("");
const popEl = ref<HTMLElement | null>(null);
const listEl = ref<HTMLElement | null>(null);
const style = ref<Record<string, string>>({});

interface Row {
  providerId: string;
  label: string;
  model: string;
}
/** 组 = 启用档；档无 models 清单时回落其单 model（不发明空组）。 */
const rows = computed<Row[]>(() =>
  props.providers
    .filter((p) => p.enabled)
    .flatMap((p) => {
      const models = p.models.length > 0 ? p.models.map((m) => m.id) : p.model ? [p.model] : [];
      return models.map((model) => ({ providerId: p.id, label: p.label, model }));
    }),
);
const visible = computed(() => {
  const needle = query.value.trim().toLowerCase();
  if (!needle) return rows.value;
  return rows.value.filter(
    (r) => r.model.toLowerCase().includes(needle) || r.label.toLowerCase().includes(needle),
  );
});
const isCurrent = (r: Row) => r.providerId === props.activeId && r.model === props.activeModel;

function place() {
  const a = props.anchor;
  const pop = popEl.value;
  if (!a || !pop) return;
  const ar = a.getBoundingClientRect();
  const pr = pop.getBoundingClientRect();
  let left = ar.right - pr.width;
  let top = ar.bottom + 6;
  left = Math.max(8, Math.min(left, window.innerWidth - pr.width - 8));
  if (top + pr.height > window.innerHeight - 8) top = Math.max(8, ar.top - pr.height - 6);
  style.value = { left: `${left}px`, top: `${top}px` };
}

function onDocMouseDown(e: MouseEvent) {
  const t = e.target as Node;
  if (popEl.value?.contains(t) || props.anchor?.contains(t)) return;
  emit("close");
}
function onKeydown(e: KeyboardEvent) {
  if (e.key === "Escape") emit("close");
}

onMounted(async () => {
  document.addEventListener("mousedown", onDocMouseDown);
  window.addEventListener("keydown", onKeydown);
  await nextTick();
  place();
  // 打开滚到当前项（必须可见）
  const cur = listEl.value?.querySelector("[aria-selected='true']");
  cur?.scrollIntoView({ block: "center" });
});
onBeforeUnmount(() => {
  document.removeEventListener("mousedown", onDocMouseDown);
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <Teleport to="body">
    <div
      ref="popEl"
      class="fixed z-[45] w-[min(320px,calc(100vw-32px))] overflow-hidden rounded-[var(--lw-r-lg)] bg-elevated shadow-[var(--lw-shadow-raised)]"
      :style="style"
      role="listbox"
      :aria-label="t('settings.modelDefaultChange')"
    >
      <div class="flex h-10 items-center gap-2 border-b border-line-subtle px-3">
        <Icon name="search" :size="13" class="shrink-0 text-ink-4" />
        <input
          v-model="query"
          class="w-full bg-transparent text-[12.5px] text-ink-1 outline-none placeholder:text-ink-5"
          :placeholder="t('settings.modelFilterModels')"
          autofocus
          spellcheck="false"
          autocomplete="off"
        />
      </div>
      <div ref="listEl" class="max-h-[min(420px,60vh)] overflow-y-auto p-1.5">
        <p v-if="visible.length === 0" class="px-2 py-4 text-center text-[12px] text-ink-4">
          {{ t("settings.modelNoMatch") }}
        </p>
        <template v-for="(r, i) in visible" :key="`${r.providerId}:${r.model}`">
          <p
            v-if="i === 0 || visible[i - 1].providerId !== r.providerId"
            class="px-2 pb-1 pt-2 text-[11px] text-ink-4"
          >
            {{ r.label }}
          </p>
          <button
            type="button"
            role="option"
            class="flex w-full items-center gap-2 rounded-[var(--lw-r-sm)] px-2 py-1.5 text-left transition hover:bg-wash-2"
            :aria-selected="isCurrent(r)"
            @click="emit('select', r.providerId, r.model); emit('close')"
          >
            <span class="w-4 shrink-0 text-accent">
              <Icon v-if="isCurrent(r)" name="check" :size="12" />
            </span>
            <span class="min-w-0 flex-1 truncate font-mono text-[12.5px] text-ink-1">{{ r.model }}</span>
          </button>
        </template>
      </div>
    </div>
  </Teleport>
</template>
