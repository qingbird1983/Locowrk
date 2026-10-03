<script setup lang="ts">
/**
 * 权限模式切换器（UI shell doc §3.33，㉘·B）——composer 左组首位的四档
 * 会话级开关（计划 / 询问 / 自动编辑 / 完全）。裁决权威在后端
 * （DESIGN §7.2 决策表），本组件只写档位（旁车持久 + 即时生效）。
 * 无活动会话（首页空态）不渲染——档位跟着会话走。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { Icon } from "../../lib/icons";

type Mode = "plan" | "ask" | "acceptEdits" | "auto";
const MODES: Mode[] = ["plan", "ask", "acceptEdits", "auto"];

const { t } = useI18n();
const chat = useChatStore();
const settings = useSettingsStore();

const open = ref(false);
const root = ref<HTMLElement | null>(null);
const errorText = ref("");

/** 当前档 = 会话显式档 → 设置缺省档（与后端 PermissionModeStore 同一回落）。 */
const current = computed<Mode>(
  () =>
    chat.permissionMode ??
    settings.view?.tools.defaultPermissionMode ??
    "ask",
);
const shortLabel = computed(() => t(`perm.short.${current.value}`));

/** 首页空态不渲染：活动会话已在盘上（列表里）或正在运行。 */
const visible = computed(
  () => chat.sessions.some((s) => s.id === chat.sessionId) || chat.running,
);

async function pick(m: Mode) {
  open.value = false;
  if (m === current.value) return;
  errorText.value = "";
  try {
    await chat.setPermissionMode(m);
  } catch (err) {
    errorText.value = t("perm.setFailed", { reason: String(err) });
  }
}

// 浮层外点关闭（§五：小浮层不挂遮罩）
function onDocPointerDown(e: PointerEvent) {
  if (!open.value) return;
  if (!(root.value?.contains(e.target as Node) ?? false)) open.value = false;
}
function onDocKeydown(e: KeyboardEvent) {
  if (open.value && e.key === "Escape") open.value = false;
}
onMounted(() => {
  document.addEventListener("pointerdown", onDocPointerDown);
  document.addEventListener("keydown", onDocKeydown);
});
onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocPointerDown);
  document.removeEventListener("keydown", onDocKeydown);
});
</script>

<template>
  <div v-if="visible" ref="root" class="perm-picker">
    <button
      class="perm-picker__btn"
      type="button"
      :aria-expanded="open"
      :aria-label="`${t('perm.title')}: ${shortLabel}`"
      @click="open = !open"
    >
      <Icon name="shield" :size="14" />
      <span class="perm-picker__label">{{ shortLabel }}</span>
    </button>

    <!-- 上弹菜单（§3.33：四行 radio，当前档 wash-3 + 勾） -->
    <div v-if="open" class="perm-picker__menu" role="listbox" :aria-label="t('perm.title')">
      <button
        v-for="m in MODES"
        :key="m"
        class="perm-picker__row"
        :class="{ 'perm-picker__row--on': m === current }"
        type="button"
        role="option"
        :aria-selected="m === current"
        @click="pick(m)"
      >
        <span class="perm-picker__mode">{{ t(`perm.short.${m}`) }}</span>
        <span class="perm-picker__desc">{{ t(`perm.desc.${m}`) }}</span>
        <Icon v-if="m === current" name="check" :size="14" class="perm-picker__check" />
      </button>
    </div>
    <p v-if="errorText" class="perm-picker__error">{{ errorText }}</p>
  </div>
</template>

<style scoped>
.perm-picker {
  position: relative;
}
.perm-picker__btn {
  display: inline-flex;
  align-items: center;
  gap: 5px;
  height: 24px;
  padding: 0 8px;
  border-radius: var(--lw-r-md);
  color: var(--lw-ink-3);
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.perm-picker__btn:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.perm-picker__btn[aria-expanded="true"] {
  background: var(--lw-wash-3);
  color: var(--lw-ink-1);
}
.perm-picker__label {
  font-size: 12.5px;
  line-height: 1;
  white-space: nowrap;
}

.perm-picker__menu {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 0;
  min-width: 224px;
  padding: 4px;
  border-radius: var(--lw-r-lg);
  background: var(--lw-surface-2);
  border: 1px solid var(--lw-line);
  box-shadow: var(--lw-shadow-raised);
  z-index: 60;
}
.perm-picker__row {
  display: flex;
  align-items: baseline;
  gap: 8px;
  width: 100%;
  padding: 7px 10px;
  border-radius: var(--lw-r-md);
  text-align: left;
}
.perm-picker__row:hover {
  background: var(--lw-wash-2);
}
.perm-picker__row--on {
  background: var(--lw-wash-3);
}
.perm-picker__mode {
  font-size: 13px;
  font-weight: 500;
  color: var(--lw-ink-1);
  white-space: nowrap;
}
.perm-picker__desc {
  font-size: 11px;
  color: var(--lw-ink-4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.perm-picker__check {
  margin-left: auto;
  align-self: center;
  color: var(--lw-ink-2);
  flex: none;
}

.perm-picker__error {
  position: absolute;
  bottom: calc(100% + 4px);
  left: 0;
  font-size: 11px;
  color: var(--lw-danger);
  white-space: nowrap;
}
</style>
