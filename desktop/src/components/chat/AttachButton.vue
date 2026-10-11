<script setup lang="ts">
/**
 * 附件按钮（UI shell doc §3.11 v3.9 + v4.24 批 D 多模态）：`＋` 圆钮 →
 * 下弹/上弹菜单（单实项「添加文件或图片」）→ `pick_files` 原生多选 →
 * **图片/PDF 按扩展名分流进附件预览条**（asset_put_path 上传链路，随消息
 * 作 parts 发出）；其余路径仍以 `@路径` 引用写进输入框（v1 语义，模型经
 * fs 工具读取）。
 */
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { pickFiles } from "../../lib/api";
import { useDismissOnOutside } from "../../lib/use-popover";
import { Icon } from "../../lib/icons";
import HoverTip from "../settings/HoverTip.vue";

const props = defineProps<{ dropDown?: boolean }>();
const emit = defineEmits<{ attach: [paths: string[]] }>();

const { t } = useI18n();
const chat = useChatStore();

const open = ref(false);
const root = ref<HTMLElement | null>(null);
/** 弹出方向与权限/模型弹层同规：首页 composer 居中 = 向下弹。 */
const dropDown = computed(() => props.dropDown ?? chat.items.length === 0);

const MEDIA_RE = /\.(png|jpe?g|webp|gif|pdf)$/i;

async function pick() {
  open.value = false;
  const paths = await pickFiles();
  if (!paths || paths.length === 0) return;
  const media: string[] = [];
  const refs: string[] = [];
  for (const p of paths) (MEDIA_RE.test(p) ? media : refs).push(p);
  for (const p of media) void chat.addAttachmentPath(p);
  if (refs.length) emit("attach", refs);
}

// 浮层外点关闭（§五：小浮层不挂遮罩）→ 唯一实现见 lib/use-popover.ts
useDismissOnOutside(root, open);
</script>

<template>
  <div ref="root" class="attach">
    <HoverTip :text="t('composer.attach')">
      <button
        class="attach__btn"
        type="button"
        :aria-expanded="open"
        :aria-label="t('composer.attach')"
        @click="open = !open"
      >
        <Icon name="plus" :size="14" aria-hidden="true" />
      </button>
    </HoverTip>

    <Transition :name="dropDown ? 'lw-pop-down' : 'lw-pop-up'">
      <div
        v-if="open"
        class="attach__menu"
        :class="{ 'attach__menu--down': dropDown }"
        role="menu"
      >
        <button class="attach__row" type="button" role="menuitem" @click="pick">
          <Icon name="paperclip" :size="14" class="attach__row-icon" aria-hidden="true" />
          <span class="attach__row-text">
            <span class="attach__mode">{{ t("composer.attach") }}</span>
            <span class="attach__desc">{{ t("composer.attachHint") }}</span>
          </span>
        </button>
      </div>
    </Transition>
  </div>
</template>

<style scoped>
.attach {
  position: relative;
  display: inline-flex;
  flex: 0 0 auto;
}
.attach__btn {
  display: inline-flex;
  width: 26px;
  height: 26px;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-full);
  color: var(--lw-ink-2);
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.attach__btn:hover,
.attach__btn[aria-expanded="true"] {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}

.attach__menu {
  position: absolute;
  bottom: calc(100% + 8px);
  left: 0;
  min-width: 224px;
  padding: var(--lw-gap-4);
  border-radius: var(--lw-r-lg);
  background: var(--lw-bg-elevated);
  border: 1px solid var(--lw-line);
  box-shadow: var(--lw-shadow-dialog);
  z-index: 60;
}
.attach__menu--down {
  top: calc(100% + 8px);
  bottom: auto;
}
.attach__row {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-10);
  width: 100%;
  padding: var(--lw-gap-8) var(--lw-gap-10);
  border-radius: var(--lw-r-md);
  text-align: left;
}
.attach__row:hover {
  background: var(--lw-wash-2);
}
.attach__row-icon {
  flex: 0 0 auto;
  color: var(--lw-ink-2);
}
.attach__row-text {
  display: flex;
  min-width: 0;
  flex: 1 1 auto;
  flex-direction: column;
  gap: var(--lw-gap-2);
}
.attach__mode {
  font-size: var(--lw-fs-13);
  font-weight: 500;
  color: var(--lw-ink-1);
  white-space: nowrap;
}
.attach__desc {
  font-size: var(--lw-fs-11);
  color: var(--lw-ink-4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
