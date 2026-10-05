<script setup lang="ts">
/**
 * 窗口控制（UI shell doc §2.2-5 / §2.1 窗口控制车道）：浮在 46px 带右端
 * 120px 车道上的三个平面字形按钮（图标 12px），z-index 1000 恒在最上
 * （§五阶梯）。**容器自身透明**——车道底色透出正下方那段带（面板打开时 =
 * 面板头的 bg-dock，原型 .wincontrols 同款）。无边框窗口（tauri.conf
 * decorations:false）的唯一关闭/极大化通路；关闭钮 hover 红底白字形
 * （v2.60 修复批，Windows 标准配置 #c42b1c）；Mac 不渲染（原生红绿灯）。
 */
import { onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Icon } from "../../lib/icons";

const { t } = useI18n();
const appWindow = getCurrentWindow();
const maximized = ref(false);
let unlisten: (() => void) | null = null;

async function refresh() {
  maximized.value = await appWindow.isMaximized().catch(() => false);
}

onMounted(async () => {
  await refresh();
  unlisten = await appWindow.onResized(() => void refresh());
});

onBeforeUnmount(() => {
  unlisten?.();
});
</script>

<template>
  <div
    class="fixed right-0 top-0 z-[1000] flex h-[var(--lw-toolbar-h)] w-[var(--lw-wincontrols)]"
  >
    <button
      class="win-btn"
      type="button"
      :title="t('window.minimize')"
      :aria-label="t('window.minimize')"
      @click="appWindow.minimize()"
    >
      <Icon name="min" :size="12" aria-hidden="true" />
    </button>
    <button
      class="win-btn"
      type="button"
      :title="maximized ? t('window.restore') : t('window.maximize')"
      :aria-label="maximized ? t('window.restore') : t('window.maximize')"
      @click="appWindow.toggleMaximize().then(refresh)"
    >
      <Icon
        :name="maximized ? 'copy' : 'max'"
        :size="maximized ? 11 : 12"
        aria-hidden="true"
      />
    </button>
    <button
      class="win-btn win-btn--close"
      type="button"
      :title="t('window.close')"
      :aria-label="t('window.close')"
      @click="appWindow.close()"
    >
      <Icon name="close" :size="12" aria-hidden="true" />
    </button>
  </div>
</template>

<style scoped>
.win-btn {
  display: inline-flex;
  height: 100%;
  min-width: 0;
  flex: 1 1 0;
  align-items: center;
  justify-content: center;
  border-radius: 0;
  color: color-mix(in oklab, var(--lw-ink-1) 55%, transparent);
}
.win-btn:hover {
  color: var(--lw-ink-1);
  background: var(--lw-wash-2);
}
/* 关闭钮（v2.60 修复批用户裁定）：Windows 标准配置——hover 红底白字形
 * （Win11 #c42b1c），按下加深一档 */
.win-btn--close:hover,
.win-btn--close:focus-visible {
  background: #c42b1c;
  color: #fff; /* stylelint-disable-line declaration-property-value-allowed-list -- 豁免§七:Win11关闭钮白字形(v2.60⑩) */
}
.win-btn--close:active {
  background: #b12a1c;
  color: #fff; /* stylelint-disable-line declaration-property-value-allowed-list -- 豁免§七:Win11关闭钮白字形(v2.60⑩) */
}
</style>
