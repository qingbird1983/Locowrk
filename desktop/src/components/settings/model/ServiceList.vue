<script setup lang="ts">
/**
 * AI 服务卡片列表（UI shell 文档 §3.30，参考 PI-Desktop ServiceList/
 * ServiceRow）：行 = 34px 字母瓦片 + 名称（+ 默认/无密钥/已停用徽章）+
 * meta（host · N 个模型）+ 右侧 SettingsToggle + … 菜单（编辑/设为默认/
 * 测试连接/复制配置/删除·两步确认）。行点击 = 编辑；HTML5 拖拽排序，
 * 落盘顺序即数组顺序；停用行降透明。纯展示 + 事件上抛，写入全在父级。
 */
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { Icon } from "../../../lib/icons";
import { hostOf, monogramLetter } from "../../../lib/provider-presets";
import RowMenu, { type RowMenuItem } from "../../shell/RowMenu.vue";
import SettingsToggle from "../SettingsToggle.vue";
import type { ProviderView } from "../../../types";

const props = defineProps<{
  providers: ProviderView[];
  /** 活动档 id（默认徽章 + 设为默认菜单项显隐）。 */
  activeId: string;
  /** 页面级写入进行中（拖拽/开关/菜单动作锁）。 */
  busy?: boolean;
}>();
const emit = defineEmits<{
  edit: [p: ProviderView];
  makeDefault: [p: ProviderView];
  test: [p: ProviderView];
  copy: [p: ProviderView];
  toggle: [p: ProviderView];
  remove: [p: ProviderView];
  /** 拖拽落定后的新顺序（整表）。 */
  reorder: [list: ProviderView[]];
}>();

const { t } = useI18n();
const menuFor = ref<ProviderView | null>(null);
const menuPos = ref({ x: 0, y: 0 });
const armedId = ref<string | null>(null);
const dragId = ref<string | null>(null);

function openMenu(p: ProviderView, e: MouseEvent) {
  armedId.value = null;
  menuPos.value = { x: e.clientX, y: e.clientY };
  menuFor.value = p;
}

function onMenuSelect(id: string) {
  const p = menuFor.value;
  if (!p) return;
  if (id === "remove") {
    // 两步确认：第一次点只武装，菜单保持开启（keepOpen）
    if (armedId.value !== p.id) {
      armedId.value = p.id;
      return;
    }
  }
  menuFor.value = null;
  if (id === "edit") emit("edit", p);
  else if (id === "default") emit("makeDefault", p);
  else if (id === "test") emit("test", p);
  else if (id === "copy") emit("copy", p);
  else if (id === "remove") emit("remove", p);
}

function menuItems(p: ProviderView): RowMenuItem[] {
  const isDefault = p.id === props.activeId;
  return [
    { id: "edit", label: t("settings.modelRowEdit") },
    ...(isDefault ? [] : [{ id: "default", label: t("settings.modelRowMakeDefault") }]),
    { id: "test", label: t("settings.modelTestConn") },
    { id: "copy", label: t("settings.modelRowCopy") },
    {
      id: "remove",
      label:
        armedId.value === p.id ? t("settings.modelRowRemoveConfirm") : t("settings.modelRowRemove"),
      danger: true,
      keepOpen: armedId.value !== p.id,
    },
  ];
}

function onDrop(target: ProviderView) {
  const list = [...props.providers];
  const from = list.findIndex((p) => p.id === dragId.value);
  const to = list.findIndex((p) => p.id === target.id);
  dragId.value = null;
  if (from === -1 || to === -1 || from === to) return;
  const [moved] = list.splice(from, 1);
  list.splice(to, 0, moved);
  emit("reorder", list);
}

const letter = (s: string) => monogramLetter(s);
const host = (u: string | null) => hostOf(u);
</script>

<template>
  <ul class="space-y-1.5">
    <li
      v-for="p in providers"
      :key="p.id"
      class="flex cursor-pointer items-center gap-3 rounded-[var(--lw-r-md-plus)] bg-wash-1 px-4 py-3 transition hover:bg-wash-2"
      :class="[!p.enabled ? 'opacity-55' : '', dragId === p.id ? 'opacity-50' : '']"
      :draggable="!busy"
      :aria-label="p.label"
      @click="emit('edit', p)"
      @dragover.prevent
      @drop.prevent="onDrop(p)"
    >
      <span
        class="flex h-[34px] w-[34px] shrink-0 items-center justify-center rounded-[var(--lw-r-xs)] bg-wash-2 text-[length:var(--lw-fs-13)] font-semibold text-ink-2"
        aria-hidden
      >
        <Icon v-if="!letter(p.label)" name="server" :size="14" />
        <template v-else>{{ letter(p.label) }}</template>
      </span>
      <span class="min-w-0 flex-1">
        <span class="flex items-center gap-1.5">
          <span class="truncate text-[length:var(--lw-fs-13)] font-medium text-ink-1">{{ p.label }}</span>
          <span
            v-if="p.id === activeId"
            class="rounded-full bg-ok/16 px-1.5 py-px text-[length:var(--lw-fs-10)] font-medium text-ok"
          >{{ t("settings.modelBadgeDefault") }}</span>
          <span
            v-if="!p.apiKeySet && p.kind !== 'ollama'"
            class="rounded-full bg-warn/16 px-1.5 py-px text-[length:var(--lw-fs-10)] font-medium text-warn"
          >{{ t("settings.modelBadgeNoKey") }}</span>
          <span
            v-if="!p.enabled"
            class="rounded-full bg-wash-3 px-1.5 py-px text-[length:var(--lw-fs-10)] font-medium text-ink-3"
          >{{ t("settings.modelBadgeDisabled") }}</span>
        </span>
        <span class="mt-0.5 block truncate text-[length:var(--lw-fs-12)] text-ink-4">
          {{ host(p.baseUrl) || "—" }} · {{ t("settings.modelCount", { n: p.models.length }) }}
        </span>
      </span>
      <span class="flex shrink-0 items-center gap-1" @click.stop>
        <SettingsToggle
          :model-value="p.enabled"
          :label="t('settings.modelEnabledToggle')"
          @update:model-value="emit('toggle', p)"
        />
        <button
          type="button"
          class="rounded-[var(--lw-r-2xs)] p-1.5 text-ink-4 transition hover:bg-wash-3 hover:text-ink-1"
          :aria-label="t('settings.modelRowActions')"
          @click="openMenu(p, $event)"
        >
          <Icon name="dots" :size="14" />
        </button>
      </span>
    </li>

    <!-- 菜单进退场（标准 §二①）：Teleport 出列表裁剪 + Transition 接管 -->
    <Teleport to="body">
      <Transition name="lw-pop-down">
        <RowMenu
          v-if="menuFor"
          :items="menuItems(menuFor)"
          :x="menuPos.x"
          :y="menuPos.y"
          @select="onMenuSelect"
          @close="menuFor = null"
        />
      </Transition>
    </Teleport>
  </ul>
</template>
