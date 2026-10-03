<script setup lang="ts">
/**
 * composer 坞（UI shell doc §3.11）：浮起卡片（r-xl、bg-composer、
 * shadow-composer、无边框），列宽与转录列同源（§2.7，不许写死——欢迎页
 * 空态变体例外，见 §3.11/§3.14）。上行 = 自适应高度文本域；工具条左组 =
 * 附件 ＋（v2.59：添加文件或图片，路径引用 v1）/ 权限模式 chip；右组 =
 * 用量环（UsageRing，§3.12）→ 模型 chip（§3.30 选择器：会话内写覆盖、
 * 首页写缺省）→ 发送键。卡片内底行 = 工作区 chip（WorkspacePicker，项目
 * 归属随手切）。收缩规则 = 左组固定不缩、右组可缩（模型 chip 先压字）。
 * Enter 发送 / Shift+Enter 换行（enterToSend 开关）；运行中忽略。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useShellStore } from "../../stores/shell";
import { useToastStore } from "../../stores/toast";
import { Icon } from "../../lib/icons";
import AttachButton from "./AttachButton.vue";
import ModelSelector from "./ModelSelector.vue";
import PermissionModePicker from "./PermissionModePicker.vue";
import UsageRing from "./UsageRing.vue";
import WorkspacePicker from "./WorkspacePicker.vue";

const emit = defineEmits<{ height: [h: number] }>();

const { t } = useI18n();
const chat = useChatStore();
const settings = useSettingsStore();
const shell = useShellStore();
const toast = useToastStore();

const draft = ref("");
const ta = ref<HTMLTextAreaElement | null>(null);
const dock = ref<HTMLElement | null>(null);
const showModels = ref(false);
/** §3.25：↑（输入框为空时）召回上一条已发送输入。 */
const lastSent = ref("");

const enterToSend = computed(() => settings.view?.chat.enterToSend ?? true);
/** 模型 chip = 会话覆盖 ?? 设置缺省（「跟随默认」出口在浮层里，chip 不加
 * 标记）。enabledProviders = 选择器数据源（已配置清单，停用档不出现）。 */
const modelLabel = computed(
  () => chat.modelOverride?.model || settings.view?.llm.model || t("app.modelFallback"),
);
const enabledProviders = computed(() => (settings.view?.llm.providers ?? []).filter((p) => p.enabled));
/** 选择语义（§3.30）：会话内 = 覆盖；首页 = 写设置缺省；写穿失败如实报错。 */
function onSelectModel(providerId: string, model: string) {
  chat
    .setModel(providerId, model)
    .catch((e) => toast.push("danger", t("model.switchFailed"), String(e)));
}

function onClearOverride() {
  chat
    .clearModelOverride()
    .catch((e) => toast.push("danger", t("model.switchFailed"), String(e)));
}

function onConfigure() {
  shell.openSettings("llm");
}

/** §3.11 v3.9 附件 v1：路径以 `@路径` 引用追加进输入框（模型经 fs 工具
 * 读取）；已有草稿时空格分隔。图片内容上传待 ㉛ 消息通路。 */
function onAttach(paths: string[]) {
  const refs = paths.map((p) => `@${p}`).join(" ");
  draft.value = draft.value ? `${draft.value} ${refs}` : refs;
}

function autoResize() {
  const el = ta.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = `${Math.min(el.scrollHeight, 180)}px`;
}
watch(draft, () => void nextTick(autoResize));

function send() {
  const text = draft.value;
  if (!text.trim() || chat.running) return;
  draft.value = "";
  lastSent.value = text;
  chat.send(text);
  void nextTick(autoResize);
}

function onKeydown(e: KeyboardEvent) {
  // §3.25：Esc = 输入框交出焦点（各浮层自持 Esc）；输入法组合中是取消候选
  if (e.key === "Escape" && !e.isComposing) {
    ta.value?.blur();
    return;
  }
  // §3.25：↑（输入框为空时）召回上一条输入
  if (e.key === "ArrowUp" && !e.isComposing && draft.value === "") {
    if (lastSent.value) {
      e.preventDefault();
      draft.value = lastSent.value;
      void nextTick(() => ta.value?.setSelectionRange(lastSent.value.length, lastSent.value.length));
    }
    return;
  }
  if (e.key !== "Enter" || e.shiftKey) return;
  // 输入法组合中的 Enter 是"确认候选"，不是发送（中文输入必备）
  if (e.isComposing) return;
  const sendKey = enterToSend.value
    ? !e.ctrlKey && !e.metaKey
    : e.ctrlKey || e.metaKey;
  if (!sendKey) return;
  e.preventDefault();
  send();
}

// §3.17：审批决策后焦点回 composer（批完接着打字）
watch(
  () => shell.composerFocusTick,
  () => {
    if (shell.destination === "chat") ta.value?.focus();
  },
);

// 坞总高上报：列宽热区下沿要停在 composer 顶部
let ro: ResizeObserver | null = null;
onMounted(() => {
  if (dock.value) {
    ro = new ResizeObserver(() => emit("height", dock.value?.offsetHeight ?? 0));
    ro.observe(dock.value);
    emit("height", dock.value.offsetHeight);
  }
});
onBeforeUnmount(() => ro?.disconnect());
</script>

<template>
  <div ref="dock" class="composer-dock">
    <div
      class="composer conv-w"
      :style="{ pointerEvents: chat.replaying ? 'none' : undefined }"
    >
      <textarea
        ref="ta"
        v-model="draft"
        rows="1"
        class="composer__input scroll-y"
        :placeholder="t('chat.placeholder')"
        @keydown="onKeydown"
      />
      <div class="composer__toolbar">
        <!-- 左组：附件 ＋（v2.59）+ 权限模式切换器（§3.33） -->
        <div class="composer__left">
          <AttachButton :drop-down="!chat.items.length" @attach="onAttach" />
          <PermissionModePicker />
        </div>
        <div class="composer__right">
          <!-- 用量环（§3.12，自持开合） -->
          <UsageRing />
          <!-- 模型 chip（§3.30 v2.58）：点开模型选择器浮层 -->
          <div class="chip-anchor">
            <button
              class="chip chip--model"
              type="button"
              :aria-expanded="showModels"
              :aria-label="t('model.selector')"
              @click="showModels = !showModels"
            >
              <Icon name="sparkle" :size="14" class="shrink-0" />
              <span class="chip__text">{{ modelLabel }}</span>
              <Icon name="chevronDown" :size="11" class="shrink-0 chip__chevron" />
            </button>
            <ModelSelector
              v-if="showModels"
              :default-model="settings.view?.llm.model ?? ''"
              :override="chat.modelOverride"
              :providers="enabledProviders"
              :drop-down="!chat.items.length"
              @select="onSelectModel"
              @clear="onClearOverride"
              @configure="onConfigure"
              @close="showModels = false"
            />
          </div>
          <button
            class="send-btn"
            type="button"
            :disabled="!draft.trim() || chat.running"
            :aria-label="
              chat.running ? t('chat.stopGeneration') : t('chat.send')
            "
            @click="send"
          >
            <Icon v-if="!chat.running" name="sendUp" :size="18" />
            <Icon v-else name="stop" :size="18" />
          </button>
        </div>
      </div>
    </div>

    <!-- 工作区 chip 行（§3.11 v3.9，照参考对象补齐）：项目归属随手切，数据
         与侧栏项目段同源；**在卡片外、坐在下方暗影层上**（用户裁定） -->
    <div class="composer__wsrow">
      <WorkspacePicker :drop-down="!chat.items.length" />
    </div>
  </div>
</template>

<style scoped>
.composer-dock {
  position: absolute;
  inset-inline: 0;
  bottom: 0;
  padding: 0 24px 16px;
  z-index: 12;
  pointer-events: none;
}
.composer {
  position: relative;
  width: min(100%, var(--lw-content-max));
  margin-inline: auto;
  border-radius: var(--lw-r-xl);
  background: var(--lw-bg-composer);
  box-shadow: var(--lw-shadow-composer);
  pointer-events: auto;
}
.composer__input {
  display: block;
  width: 100%;
  border: 0;
  outline: none;
  background: none;
  resize: none;
  overflow-y: auto;
  border-radius: var(--lw-r-xl) var(--lw-r-xl) 0 0;
  padding: 12px 14px 4px;
  font-size: 14px;
  line-height: 1.45;
  max-height: 180px;
}
.composer__input::placeholder {
  color: var(--lw-ink-5);
  font-size: 12.5px;
}
.composer__toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px 6px;
}
/* 工作区 chip 行（§3.11 v3.9 用户裁定）：卡片**外**底行，坐在下方暗影
 * 层上——宽度与卡同源，dock pointer-events:none 需显式放开 */
.composer__wsrow {
  display: flex;
  align-items: center;
  gap: 8px;
  width: min(100%, var(--lw-content-max));
  margin-inline: auto;
  padding: 6px 14px 0;
  pointer-events: auto;
}
/* 收缩规则（§3.11 v3.8）：左组固定不缩；右组可缩——窄窗口下先压模型
 * chip 的文字，而不是溢出卡片 */
.composer__left,
.composer__right {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}
.composer__left {
  flex: 0 0 auto;
}
.composer__right {
  flex: 0 1 auto;
  margin-left: auto;
  justify-content: flex-end;
}
/* §3.30 锚点：模型选择器浮层锚在 chip 上方（右对齐 chip），不随滚动裁切
 * ——composer 卡片 overflow 可见，浮层 z 高于用量浮层 */
.chip-anchor {
  position: relative;
  display: inline-flex;
  min-width: 0;
}
.chip-anchor .chip--model {
  cursor: pointer;
}
.chip-anchor .chip__chevron {
  color: var(--lw-ink-4);
}
.chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  height: 28px;
  padding: 4px 10px;
  border-radius: var(--lw-r-full);
  background: var(--lw-wash-2);
  font-size: 12.5px;
  line-height: 1.2;
  color: var(--lw-ink-2);
  white-space: nowrap;
}
.chip > svg {
  color: var(--lw-ink-3);
}
.chip--model {
  flex: 0 1 auto;
  max-width: 260px;
  /* v2.59 用户裁定：chip 底色不常显（与权限 chip 同款），hover/展开才显 */
  background: transparent;
}
.chip-anchor:hover .chip--model,
.chip--model[aria-expanded="true"] {
  background: var(--lw-wash-2);
}
.chip__text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.send-btn {
  width: 36px;
  height: 36px;
  border-radius: var(--lw-r-full);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  background: var(--lw-accent);
  color: var(--lw-accent-ink);
  flex: 0 0 auto;
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard),
    transform var(--dur-instant) var(--ease-standard);
}
.send-btn:hover:not(:disabled) {
  background: var(--lw-accent-hover);
}
.send-btn:active:not(:disabled) {
  transform: scale(0.96);
}
.send-btn:disabled {
  /* 禁用态（v2.59 用户裁定「互换一下」）：底加深、箭头变浅——底比箭深 */
  background: color-mix(in oklab, var(--lw-ink-1) 32%, transparent);
  color: color-mix(in oklab, var(--lw-ink-1) 6%, transparent);
  cursor: default;
}
</style>
