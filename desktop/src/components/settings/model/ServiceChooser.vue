<script setup lang="ts">
/**
 * 选服务瓦片格（UI shell 文档 §3.30 添加/编辑服务模态·选服务视图，参考
 * PI-Desktop ServiceChooser）：顶部搜索行 + 「用 API KEY 连接」瓦片格
 * `repeat(auto-fill, minmax(220px, 1fr))`——自定义端点居首（＋ 字形瓦片），
 * 其后为内置预设（字母瓦片 + 名称 + baseUrl host detail）。过滤纯前端
 * 子串匹配（haystack 覆盖名称/id/别名/host）；↑↓←→ 网格导航、Enter 选
 * 首个命中、↑ 从首行回搜索框；无命中 → 「用自定义端点」动作。
 * （PI 的「用订阅登录」组 = OAuth，本项目无此后端，整组不做。）
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../../lib/icons";
import {
  customOption,
  CUSTOM_SERVICE,
  filterOptions,
  monogramLetter,
  presetOptions,
  type ServiceOption,
} from "../../../lib/provider-presets";

const props = defineProps<{
  /** 表单当前服务 id（返回 chooser 时 aria-current 标记）。 */
  current?: string;
  disabled?: boolean;
}>();
const emit = defineEmits<{ pick: [id: string] }>();

const { t } = useI18n();
const query = ref("");
const searchEl = ref<HTMLInputElement | null>(null);
const gridEl = ref<HTMLElement | null>(null);

const custom = computed(() => {
  const o = customOption();
  o.name = t("settings.presetCustom");
  o.haystack = `${o.name} ${o.haystack}`.toLowerCase();
  return o;
});
const visible = computed(() =>
  filterOptions([custom.value, ...presetOptions()], query.value),
);

function pickEnterTarget() {
  if (visible.value[0]) emit("pick", visible.value[0].id);
  else emit("pick", CUSTOM_SERVICE);
}

function tiles(): HTMLButtonElement[] {
  return [...(gridEl.value?.querySelectorAll<HTMLButtonElement>("[data-tile]") ?? [])];
}

/** ←→ 顺读序；↑↓ 走几何最近行（网格列数随宽变）；首行 ↑ 回搜索框。 */
function onTileKey(e: KeyboardEvent) {
  const all = tiles();
  const i = all.indexOf(e.currentTarget as HTMLButtonElement);
  if (i === -1) return;
  let next: HTMLButtonElement | undefined;
  if (e.key === "ArrowRight") next = all[i + 1];
  else if (e.key === "ArrowLeft") next = all[i - 1];
  else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    const down = e.key === "ArrowDown";
    const origin = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const cx = origin.left + origin.width / 2;
    let best = Number.POSITIVE_INFINITY;
    for (const tile of all) {
      const box = tile.getBoundingClientRect();
      const gap = down ? box.top - origin.bottom : origin.top - box.bottom;
      if (gap < -1) continue;
      const score = gap * 1000 + Math.abs(box.left + box.width / 2 - cx);
      if (score < best) {
        best = score;
        next = tile;
      }
    }
    if (!next && !down) {
      e.preventDefault();
      searchEl.value?.focus();
      return;
    }
  } else return;
  if (next) {
    e.preventDefault();
    next.focus();
  }
}

const letter = (name: string) => monogramLetter(name);
const isCustom = (o: ServiceOption) => o.id === CUSTOM_SERVICE;
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col gap-3">
    <div
      class="flex h-10 shrink-0 items-center gap-2 rounded-[var(--lw-r-md)] bg-wash-1 px-3"
    >
      <Icon name="search" :size="14" class="shrink-0 text-ink-4" />
      <input
        ref="searchEl"
        v-model="query"
        class="w-full bg-transparent text-[length:var(--lw-fs-13)] text-ink-1 outline-none placeholder:text-ink-5"
        :placeholder="t('settings.modelSearchService')"
        :disabled="disabled"
        autocomplete="off"
        spellcheck="false"
        @keydown.enter.prevent="pickEnterTarget"
        @keydown.arrow-down.prevent="tiles()[0]?.focus()"
      />
    </div>

    <div ref="gridEl" class="min-h-0 flex-1 overflow-y-auto pb-1">
      <p
        v-if="visible.length === 0"
        class="px-1 py-6 text-center text-[length:var(--lw-fs-12half)] text-ink-4"
      >
        {{ t("settings.modelNoServiceMatch") }}
        <button
          type="button"
          class="ml-1 text-accent underline-offset-2 hover:underline"
          :disabled="disabled"
          @click="emit('pick', CUSTOM_SERVICE)"
        >
          {{ t("settings.presetCustom") }}
        </button>
      </p>
      <template v-else>
        <h4 class="mb-2 px-1 text-[length:var(--lw-fs-12half)] font-semibold text-ink-2">
          {{ t("settings.modelChooserGroup") }}
        </h4>
        <div class="grid grid-cols-[repeat(auto-fill,minmax(220px,1fr))] gap-2">
          <button
            v-for="o in visible"
            :key="o.id"
            type="button"
            data-tile
            class="flex items-center gap-2.5 rounded-[var(--lw-r-md)] bg-wash-1 px-3 py-2.5 text-left transition hover:bg-wash-2 disabled:opacity-50"
            :class="o.id === current ? 'ring-1 ring-line-strong' : ''"
            :disabled="disabled"
            :aria-current="o.id === current ? 'true' : undefined"
            @click="emit('pick', o.id)"
            @keydown="onTileKey"
          >
            <span
              class="flex h-9 w-9 shrink-0 items-center justify-center rounded-[var(--lw-r-xs)] bg-wash-2 text-[length:var(--lw-fs-14)] font-semibold text-ink-2"
              aria-hidden
            >
              <Icon v-if="isCustom(o)" name="plus" :size="14" />
              <template v-else>{{ letter(o.name) }}</template>
            </span>
            <span class="min-w-0">
              <span class="block truncate text-[length:var(--lw-fs-13)] font-medium text-ink-1">{{ o.name }}</span>
              <span class="block truncate text-[length:var(--lw-fs-11)] text-ink-4">
                {{ isCustom(o) ? t("settings.presetCustomDesc") : o.endpoint }}
              </span>
            </span>
          </button>
        </div>
      </template>
    </div>
  </div>
</template>
