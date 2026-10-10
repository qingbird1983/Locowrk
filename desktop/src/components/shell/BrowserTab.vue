<script setup lang="ts">
/**
 * 浏览器页签（UI shell 文档 §2.5 浏览器页签，v2.144）：预览页 = iframe
 * 挂 lwpreview 协议（工作区文件）或 http(s) 直挂。生命周期对齐 ZCode 的
 * 常驻纪律：面板内切页签 v-show 保活（页面状态不丢），只有显式关签才
 * 销毁；关面板/切目的地仍会卸载（App 壳 v-if，已知边界记档）。安全 =
 * iframe sandbox（无 allow-top-navigation / allow-popups，页面翻不了
 * 壳窗也弹不出新窗）；console 桶由注入桥 postMessage 回传（e.source 比
 * 对本签 iframe，兄弟签互不串）。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { invoke } from "@tauri-apps/api/core";
import type { WorkPanelTab } from "../../stores/work-panel";
import { useBrowserStore, type BrowserConsoleEntry } from "../../stores/browser";
import { isHttpUrl, previewUrlFor } from "../../lib/preview-url";
import { Icon } from "../../lib/icons";

const props = defineProps<{ tab: WorkPanelTab }>();

const { t } = useI18n();
const browserStore = useBrowserStore();

/** 导航栈（自管：跨源 iframe 的 contentWindow.history 不可触）。 */
interface NavEntry {
  url: string;
  target: string;
}
const stack = ref<NavEntry[]>([]);
const cursor = ref(-1);
const draft = ref(props.tab.target ?? "");
const reloadKey = ref(0);
const showConsole = ref(false);
const iframeEl = ref<HTMLIFrameElement | null>(null);

const current = computed(() => stack.value[cursor.value] ?? null);
const currentUrl = computed(() => current.value?.url ?? "");
const entries = computed(() => browserStore.entries[props.tab.id] ?? []);
const errorCount = computed(
  () => entries.value.filter((e) => e.level === "error").length,
);

watch(
  () => props.tab.url,
  (url) => {
    // 工具对同签重开 = 换页（store openBrowserTab 覆写同 id 的 url）
    if (url && url !== currentUrl.value) {
      navigateTo({ url, target: props.tab.target ?? url });
    }
  },
);

function navigateTo(entry: NavEntry) {
  stack.value = stack.value.slice(0, cursor.value + 1);
  stack.value.push(entry);
  cursor.value = stack.value.length - 1;
  draft.value = entry.target;
}

/** URL 栏 / 空态输入提交：http(s) 直挂，其余按工作区绝对路径走预览协议。 */
function submit() {
  const raw = draft.value.trim();
  if (!raw) return;
  if (isHttpUrl(raw)) {
    navigateTo({ url: raw, target: raw });
  } else {
    navigateTo({ url: previewUrlFor(raw), target: raw });
  }
  reloadKey.value++;
}

function goBack() {
  if (cursor.value > 0) cursor.value--;
}
function goForward() {
  if (cursor.value < stack.value.length - 1) cursor.value++;
}
function reload() {
  if (!currentUrl.value) return;
  reloadKey.value++;
}
function openExternal() {
  const target = current.value?.target ?? "";
  if (isHttpUrl(target)) void invoke("open_external_url", { url: target });
}

function onMessage(e: MessageEvent) {
  const data = e.data as { __lwPreview?: boolean; level?: string; text?: string; ts?: number } | null;
  if (!data || data.__lwPreview !== true) return;
  if (e.source !== iframeEl.value?.contentWindow) return;
  const level = data.level === "warn" ? "warn" : "error";
  const entry: BrowserConsoleEntry = {
    ts: typeof data.ts === "number" ? data.ts : Date.now(),
    level,
    text: String(data.text ?? "").slice(0, 2000),
  };
  browserStore.push(props.tab.id, entry);
}

onMounted(() => {
  window.addEventListener("message", onMessage);
  // 工具开签自带 URL：初始化导航栈（watch 不触达首挂）
  if (props.tab.url) {
    stack.value = [{ url: props.tab.url, target: props.tab.target ?? props.tab.url }];
    cursor.value = 0;
    draft.value = stack.value[0].target;
  }
});
onBeforeUnmount(() => {
  window.removeEventListener("message", onMessage);
  browserStore.drop(props.tab.id);
});

function fmtTime(ts: number): string {
  const d = new Date(ts);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`;
}
</script>

<template>
  <div class="flex min-h-0 flex-1 flex-col">
    <template v-if="currentUrl">
      <!-- 工具条：自管后退/前进 + URL 栏 + 刷新 + 系统浏览器 + 控制台 -->
      <div class="flex h-9 shrink-0 items-center gap-1 border-b border-line px-2">
        <button
          class="bctl"
          type="button"
          :disabled="cursor <= 0"
          :aria-label="t('panel.browserBack')"
          @click="goBack"
        >
          <Icon name="arrowLeft" :size="14" aria-hidden="true" />
        </button>
        <button
          class="bctl"
          type="button"
          :disabled="cursor >= stack.length - 1"
          :aria-label="t('panel.browserForward')"
          @click="goForward"
        >
          <Icon name="arrowRight" :size="14" aria-hidden="true" />
        </button>
        <input
          v-model="draft"
          class="burl"
          type="text"
          spellcheck="false"
          :placeholder="t('panel.browserUrlPlaceholder')"
          :aria-label="t('panel.browserUrlPlaceholder')"
          @keydown.enter="submit()"
        />
        <button
          class="bctl"
          type="button"
          :aria-label="t('panel.browserReload')"
          @click="reload()"
        >
          <Icon name="refresh" :size="14" aria-hidden="true" />
        </button>
        <button
          v-if="isHttpUrl(current?.target ?? '')"
          class="bctl"
          type="button"
          :aria-label="t('panel.browserExternal')"
          @click="openExternal()"
        >
          <Icon name="external" :size="14" aria-hidden="true" />
        </button>
        <button
          class="bctl relative"
          type="button"
          :data-active="showConsole || undefined"
          :aria-label="t('panel.browserConsole')"
          @click="showConsole = !showConsole"
        >
          <Icon name="terminal" :size="14" aria-hidden="true" />
          <span
            v-if="errorCount > 0"
            class="bctl-badge"
            aria-hidden="true"
          >{{ errorCount > 99 ? "99+" : errorCount }}</span>
        </button>
      </div>

      <!-- 预览面：sandbox 断 top-navigation/popups；localStorage/WebGL 由
           allow-same-origin/allow-scripts 承载（游戏可存档） -->
      <iframe
        ref="iframeEl"
        :key="`${tab.id}:${reloadKey}`"
        :src="currentUrl"
        class="min-h-0 w-full flex-1 border-0 bg-white"
        sandbox="allow-scripts allow-same-origin allow-forms allow-modals allow-pointer-lock"
        :title="tab.title"
      />

      <!-- 控制台抽屉：注入桥回传的 error/warn（页签内时间序，最新在底） -->
      <div v-if="showConsole" class="console-drawer scroll-y">
        <p v-if="entries.length === 0" class="px-3 py-2 text-[length:var(--lw-fs-12)] text-ink-4">
          {{ t("panel.browserConsoleEmpty") }}
        </p>
        <div
          v-for="(e, i) in entries"
          :key="`${e.ts}-${i}`"
          class="console-row"
          :data-level="e.level"
        >
          <span class="console-time">{{ fmtTime(e.ts) }}</span>
          <span class="console-level">{{ e.level }}</span>
          <span class="console-text">{{ e.text }}</span>
        </div>
      </div>
    </template>

    <!-- 空态（手动签 / 待输入）：路径或地址入口 -->
    <div v-else class="flex min-h-0 flex-1 flex-col items-center justify-center gap-3 px-6">
      <Icon name="globe" :size="28" aria-hidden="true" />
      <form
        class="flex w-full max-w-[420px] items-center gap-2"
        @submit.prevent="submit()"
      >
        <input
          v-model="draft"
          class="burl h-8"
          type="text"
          spellcheck="false"
          :placeholder="t('panel.browserUrlPlaceholder')"
          :aria-label="t('panel.browserUrlPlaceholder')"
        />
        <button class="bctl bctl-go" type="submit">{{ t("panel.browserGo") }}</button>
      </form>
      <p class="max-w-[420px] text-center text-[length:var(--lw-fs-12)] leading-relaxed text-ink-4">
        {{ t("panel.browserEmptyHint") }}
      </p>
    </div>
  </div>
</template>

<style scoped>
.bctl {
  display: inline-flex;
  height: 26px;
  min-width: 26px;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-sm);
  padding: 0 var(--lw-gap-4);
  color: var(--lw-ink-2);
}
.bctl:hover:not(:disabled) {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.bctl:disabled {
  opacity: 0.35;
}
.bctl[data-active] {
  background: var(--lw-wash-3);
  color: var(--lw-ink-1);
}
.bctl-go {
  font-size: var(--lw-fs-12half);
  background: var(--lw-wash-2);
  padding: 0 var(--lw-gap-10);
}
.bctl-badge {
  position: absolute;
  top: -3px;
  right: -3px;
  min-width: 14px;
  height: 14px;
  border-radius: var(--lw-r-full);
  background: var(--lw-danger);
  color: var(--lw-ink-1);
  font-size: var(--lw-fs-10);
  line-height: 14px;
  padding: 0 var(--lw-gap-4);
  text-align: center;
}

.burl {
  min-width: 0;
  flex: 1 1 auto;
  height: 26px;
  border-radius: var(--lw-r-sm);
  background: var(--lw-wash-1);
  padding: 0 var(--lw-gap-8);
  font-size: var(--lw-fs-12);
  color: var(--lw-ink-1);
}
.burl:focus {
  outline: none;
  background: var(--lw-wash-2);
}
.burl::placeholder {
  color: var(--lw-ink-4);
}

.console-drawer {
  flex: 0 0 auto;
  max-height: 40%;
  border-top: 1px solid var(--lw-line);
  background: var(--lw-wash-1);
  font-size: var(--lw-fs-12);
}
.console-row {
  display: flex;
  gap: var(--lw-gap-8);
  padding: 3px var(--lw-gap-10);
  border-bottom: 1px solid color-mix(in oklab, var(--lw-wash-3) 60%, transparent);
}
.console-row:last-child {
  border-bottom: none;
}
.console-time {
  flex: 0 0 auto;
  color: var(--lw-ink-4);
  font-variant-numeric: tabular-nums;
}
.console-level {
  flex: 0 0 auto;
  font-weight: 600;
}
.console-row[data-level="error"] .console-level {
  color: var(--lw-danger);
}
.console-row[data-level="warn"] .console-level {
  color: var(--lw-warn);
}
.console-text {
  min-width: 0;
  overflow-wrap: anywhere;
  white-space: pre-wrap;
  color: var(--lw-ink-2);
}
</style>
