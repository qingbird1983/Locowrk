<script setup lang="ts">
/**
 * 工作区 chip（UI shell doc §3.11 v3.9，用户 2026-10-04 裁定照参考对象补齐
 * ——「功能实际就是左栏的项目工作区同功能」）：composer 卡内底行的项目
 * 归属选择器。数据与侧栏项目段同源（fsRoots ∪ 已关联会话，最近优先）。
 * 菜单 = 「最近」组 + 选择新项目（pick_directory → fsRoots）/ 不需要项目
 * （清除归属）；会话内选择 = `session_associate`，首页 = 写挂起归属
 * （首聊自动关联，与项目行 ＋ 同一语义）。首屏向下弹、会话态向上弹
 * （与权限/模型弹层同规）。v2.60 修复批：`iconOnly` 变体 = 会话态顶栏
 * 标题前的纯图标触发钮（项目 chip 自 composer 底行移来，只显示图标，
 * 菜单下弹）。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useToastStore } from "../../stores/toast";
import { pickDirectory } from "../../lib/api";
import { workspaceDisplayName } from "../../lib/session-groups";
import { Icon, type IconName } from "../../lib/icons";
import HoverTip from "../settings/HoverTip.vue";

const props = defineProps<{ dropDown?: boolean; iconOnly?: boolean }>();

const { t } = useI18n();
const chat = useChatStore();
const settings = useSettingsStore();
const toast = useToastStore();

const open = ref(false);
const root = ref<HTMLElement | null>(null);

/** 会话态 = 活动会话已在盘上（列表里）或正在运行。 */
const inSession = computed(
  () => chat.sessions.some((s) => s.id === chat.sessionId) || chat.running,
);

/** 当前归属：chat.currentWorkspace 同源真源（v2.61——会话态 = 旁车
 * workspace；首页 = 挂起归属；与欢迎页问候共用一个 getter，永远同变）。 */
const currentWs = computed(() => chat.currentWorkspace);
const currentName = computed(() =>
  currentWs.value ? workspaceDisplayName(currentWs.value) : t("workspace.chip"),
);

/** 候选 = fsRoots ∪ 已关联会话的 workspace（fsRoots 序打底，会话归属按
 * 最近活动补前——与侧栏项目段同一组集合口径）。 */
const candidates = computed(() => {
  const roots = settings.view?.tools.fsRoots ?? [];
  const out = [...roots];
  for (const s of chat.sessions) {
    if (s.workspace && !out.includes(s.workspace)) out.push(s.workspace);
  }
  return out;
});

function toggle() {
  open.value = !open.value;
}

/** 选择归属（v2.59 用户裁定：**工作区要共存**）——选中另一个项目 =
 * **开一个新任务挂到它**，当前任务原地留在原项目（不许搬会话；显式搬移
 * 走侧栏行菜单「移入工作区」）。首页 = 写挂起归属（首聊自动关联）。 */
async function choose(ws: string | null) {
  open.value = false;
  if (ws === currentWs.value) return;
  if (chat.running) {
    // 当前轮在跑：切走会丢视图，如实提示（不静默吞）
    toast.push("warn", t("workspace.switchBusy"));
    return;
  }
  if (inSession.value) {
    chat.newSession(ws);
  } else {
    chat.setPendingWorkspace(ws);
  }
}

/** 选择新项目 = 原生目录选择 → 追加 fsRoots（保存即生效）→ 选中它。 */
async function addAndChoose() {
  open.value = false;
  const dir = await pickDirectory();
  if (!dir) return;
  const roots = settings.view?.tools.fsRoots ?? [];
  if (!roots.includes(dir)) {
    try {
      await settings.saveTools({ fsRoots: [...roots, dir] });
    } catch (e) {
      toast.push("danger", t("toast.projectAddFailed"), String(e));
      return;
    }
  }
  await choose(dir);
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

const chipIcon: IconName = "folder";
</script>

<template>
  <div ref="root" class="ws-picker">
    <!-- iconOnly 变体（v2.60 修复批）：顶栏标题前的纯图标触发钮 -->
    <HoverTip :text="currentWs ?? t('workspace.title')">
      <button
        v-if="props.iconOnly"
        class="ws-picker__icon-btn"
        type="button"
        :aria-expanded="open"
        :aria-label="t('workspace.title')"
        @click="toggle"
      >
        <Icon :name="chipIcon" :size="15" aria-hidden="true" />
      </button>
      <button
        v-else
        class="ws-picker__chip"
        type="button"
        :aria-expanded="open"
        :aria-label="t('workspace.title')"
        @click="toggle"
      >
        <Icon :name="chipIcon" :size="14" aria-hidden="true" />
        <span class="ws-picker__label">{{ currentName }}</span>
      </button>
    </HoverTip>

    <Transition :name="props.dropDown ? 'lw-pop-down' : 'lw-pop-up'">
      <div
        v-if="open"
        class="ws-picker__menu"
        :class="{ 'ws-picker__menu--down': props.dropDown }"
        role="listbox"
        :aria-label="t('workspace.title')"
      >
        <div class="ws-picker__group">{{ t("workspace.recent") }}</div>
        <HoverTip
          v-for="ws in candidates"
          :key="ws"
          class="w-full"
          :text="ws"
          :label="ws"
        >
          <button
            class="ws-picker__row"
            type="button"
            role="option"
            :aria-selected="ws === currentWs"
            @click="choose(ws)"
          >
            <Icon name="folder" :size="15" class="ws-picker__row-icon" aria-hidden="true" />
            <span class="ws-picker__name">{{ workspaceDisplayName(ws) }}</span>
            <Icon
              v-if="ws === currentWs"
              name="check"
              :size="14"
              class="ws-picker__check"
            />
          </button>
        </HoverTip>
        <div class="ws-picker__sep" role="separator" />
        <button class="ws-picker__row" type="button" role="option" @click="addAndChoose">
          <Icon name="folderOpen" :size="15" class="ws-picker__row-icon" aria-hidden="true" />
          <span class="ws-picker__name">{{ t("workspace.pickNew") }}</span>
        </button>
        <button
          class="ws-picker__row"
          type="button"
          role="option"
          @click="choose(null)"
        >
          <Icon name="close" :size="15" class="ws-picker__row-icon" aria-hidden="true" />
          <span class="ws-picker__name">{{ t("workspace.none") }}</span>
        </button>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.ws-picker {
  position: relative;
}
/* iconOnly 变体（v2.60）：顶栏 §3.19 方形图标控件规格，无底色 hover 才显 */
.ws-picker__icon-btn {
  display: inline-flex;
  width: var(--lw-ctl);
  height: var(--lw-ctl);
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-md);
  color: var(--lw-ink-2);
}
.ws-picker__icon-btn:hover,
.ws-picker__icon-btn[aria-expanded="true"] {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.ws-picker__chip {
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
.ws-picker__chip:hover,
.ws-picker__chip[aria-expanded="true"] {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.ws-picker__label {
  font-size: var(--lw-fs-12half);
  line-height: 1;
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ws-picker__menu {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 0;
  min-width: 216px;
  padding: var(--lw-gap-4);
  border-radius: var(--lw-r-lg);
  background: var(--lw-bg-elevated);
  border: 1px solid var(--lw-line);
  box-shadow: var(--lw-shadow-dialog);
  z-index: 60;
}
.ws-picker__menu--down {
  top: calc(100% + 6px);
  bottom: auto;
}
.ws-picker__group {
  padding: var(--lw-gap-6) var(--lw-gap-10) 3px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值3,§二⑬) */
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-4);
}
.ws-picker__row {
  display: flex;
  align-items: center;
  gap: 9px; /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值9,§二⑬) */
  width: 100%;
  padding: 7px var(--lw-gap-10); /* stylelint-disable-line declaration-property-value-disallowed-list -- 光学修正(孤值7,§二⑬) */
  border-radius: var(--lw-r-md);
  text-align: left;
}
.ws-picker__row:hover:not(:disabled) {
  background: var(--lw-wash-2);
}
.ws-picker__row:disabled {
  opacity: 0.45;
  cursor: default;
}
.ws-picker__row-icon {
  flex: 0 0 auto;
  color: var(--lw-ink-3);
}
.ws-picker__name {
  min-width: 0;
  flex: 1 1 auto;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--lw-fs-12half);
  color: var(--lw-ink-1);
}
.ws-picker__check {
  margin-left: auto;
  color: var(--lw-ink-2);
  flex: none;
}
/* 组间淡发丝线（两端不到端，§五 弹层质感） */
.ws-picker__sep {
  height: 1px;
  margin: var(--lw-gap-4) var(--lw-gap-12);
  background: var(--lw-line-subtle);
}
</style>
