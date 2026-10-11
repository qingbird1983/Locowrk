<script setup lang="ts">
/**
 * 权限模式切换器（UI shell doc §3.33，㉘·B）——composer 左组首位的四档
 * 会话级开关。裁决权威在后端（DESIGN §7.2 决策表），本组件只写档位
 * （旁车持久 + 即时生效）。首页空态照常显示（v2.59 用户裁定）：首页选择
 * = 写「新会话缺省档」（settings.tools.defaultPermissionMode，与模型 chip
 * 的首页语义同构）；会话内 = 写会话级覆盖。菜单形态照参考对象照抄
 * （v2.59 用户裁定）：四行 = 档位图标 + 名称 + 描述，行间淡发丝线，
 * 选中态**只挂行尾小对勾**（不做整行高亮）；「完全访问」为最危险档，
 * chip 图标+文字挂橘黄警示色（--lw-warn）。首屏 composer 居中 = 向下弹，
 * 会话态贴底 = 向上弹（模型选择器同规）。
 */
import { computed, ref, Transition } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { Icon, type IconName } from "../../lib/icons";
import { useDismissOnOutside } from "../../lib/use-popover";

type Mode = "plan" | "ask" | "acceptEdits" | "auto";
const MODES: Mode[] = ["plan", "ask", "acceptEdits", "auto"];
/** 档位图标（照抄参考）：💡计划模式 ✋变更前确认 🛡✓自动编辑 🛡!完全访问 */
const MODE_ICONS: Record<Mode, IconName> = {
  plan: "bulb",
  ask: "hand",
  acceptEdits: "shieldCheck",
  auto: "shieldAlert",
};
/** 最危险档（chip 警示色判据）。 */
const RISKIEST: Mode = "auto";

const { t } = useI18n();
const chat = useChatStore();
const settings = useSettingsStore();

const open = ref(false);
const root = ref<HTMLElement | null>(null);
const errorText = ref("");

/** 会话态 = 活动会话已在盘上（列表里）或正在运行；首页空态反之。 */
const inSession = computed(
  () => chat.sessions.some((s) => s.id === chat.sessionId) || chat.running,
);
/** 弹出方向：首页 composer 居中 = 向下弹；会话态贴底 = 向上弹。 */
const dropDown = computed(() => !inSession.value);

/** 当前档 = 会话显式档 → 设置缺省档（与后端 PermissionModeStore 同一回落）。 */
const current = computed<Mode>(
  () =>
    chat.permissionMode ??
    settings.view?.tools.defaultPermissionMode ??
    "ask",
);
const shortLabel = computed(() => t(`perm.short.${current.value}`));

async function pick(m: Mode) {
  open.value = false;
  if (m === current.value) return;
  errorText.value = "";
  try {
    if (!inSession.value) {
      // 首页 = 写新会话缺省档（后端校验四档取值）
      await settings.saveTools({ defaultPermissionMode: m });
    } else {
      await chat.setPermissionMode(m);
    }
  } catch (err) {
    errorText.value = t("perm.setFailed", { reason: String(err) });
  }
}

// 浮层外点关闭（§五：小浮层不挂遮罩）→ 唯一实现见 lib/use-popover.ts
useDismissOnOutside(root, open);
</script>

<template>
  <div ref="root" class="perm-picker">
    <button
      class="perm-picker__btn"
      :class="{ 'perm-picker__btn--warn': current === RISKIEST }"
      type="button"
      :aria-expanded="open"
      :aria-label="`${t('perm.title')}: ${shortLabel}`"
      @click="open = !open"
    >
      <Icon :name="MODE_ICONS[current]" :size="14" />
      <span class="perm-picker__label">{{ shortLabel }}</span>
    </button>

    <!-- 档位菜单（照参考对象）：图标+名称+描述，行间发丝线，选中只挂勾 -->
    <Transition :name="dropDown ? 'lw-pop-down' : 'lw-pop-up'">
    <div
      v-if="open"
      class="perm-picker__menu"
      :class="{ 'perm-picker__menu--down': dropDown }"
      role="listbox"
      :aria-label="t('perm.title')"
    >
      <template v-for="(m, i) in MODES" :key="m">
        <div v-if="i > 0" class="perm-picker__sep" role="separator" />
        <button
          class="perm-picker__row"
          type="button"
          role="option"
          :aria-selected="m === current"
          @click="pick(m)"
        >
          <Icon :name="MODE_ICONS[m]" :size="14" class="perm-picker__row-icon" />
          <span class="perm-picker__row-text">
            <span class="perm-picker__mode">{{ t(`perm.short.${m}`) }}</span>
            <span class="perm-picker__desc">{{ t(`perm.desc.${m}`) }}</span>
          </span>
          <Icon
            v-if="m === current"
            name="check"
            :size="14"
            class="perm-picker__check"
          />
        </button>
      </template>
    </div>
    </Transition>
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
  gap: 5px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值5,§二⑬) */
  height: 24px;
  padding: 0 var(--lw-gap-8);
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
/* 最危险档 = 橘黄警示色（图标+文字，用户裁定） */
.perm-picker__btn--warn {
  color: var(--lw-warn);
}
.perm-picker__btn--warn:hover,
.perm-picker__btn--warn[aria-expanded="true"] {
  background: color-mix(in oklab, var(--lw-warn) 10%, transparent);
  color: var(--lw-warn);
}
.perm-picker__label {
  font-size: var(--lw-fs-12half);
  line-height: 1;
  white-space: nowrap;
}

.perm-picker__menu {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 0;
  min-width: 224px;
  padding: var(--lw-gap-4);
  border-radius: var(--lw-r-lg);
  background: var(--lw-bg-elevated);
  border: 1px solid var(--lw-line);
  box-shadow: var(--lw-shadow-dialog);
  z-index: 60;
}
/* 首屏 composer 居中：向下弹（v2.59 用户裁定，模型选择器同规） */
.perm-picker__menu--down {
  top: calc(100% + 6px);
  bottom: auto;
}
.perm-picker__row {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-10);
  width: 100%;
  padding: var(--lw-gap-8) var(--lw-gap-10);
  border-radius: var(--lw-r-md);
  text-align: left;
}
.perm-picker__row:hover {
  background: var(--lw-wash-2);
}
.perm-picker__row-icon {
  flex: 0 0 auto;
  color: var(--lw-ink-2);
}
.perm-picker__row-text {
  display: flex;
  min-width: 0;
  flex: 1 1 auto;
  flex-direction: column;
  gap: var(--lw-gap-2);
}
.perm-picker__mode {
  font-size: var(--lw-fs-13);
  font-weight: 500;
  color: var(--lw-ink-1);
  white-space: nowrap;
}
.perm-picker__desc {
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
/* 选中态只挂行尾小对勾（v2.59 用户裁定：整行高亮不做） */
.perm-picker__check {
  margin-left: auto;
  align-self: center;
  color: var(--lw-ink-2);
  flex: none;
}
/* 行间淡发丝线（两端不到端，§五 弹层质感） */
.perm-picker__sep {
  height: 1px;
  margin: var(--lw-gap-4) var(--lw-gap-12);
  background: var(--lw-line-subtle);
}

.perm-picker__error {
  position: absolute;
  bottom: calc(100% + 4px);
  left: 0;
  font-size: var(--lw-fs-11);
  color: var(--lw-danger);
  white-space: nowrap;
}
</style>
