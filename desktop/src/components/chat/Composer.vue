<script setup lang="ts">
/**
 * composer 坞（UI shell doc §3.11 v4.11）：浮起卡片（r-xl、bg-composer、
 * shadow-composer、无边框），列宽与转录列同源（§2.7，不许写死——欢迎页
 * 空态变体例外，见 §3.11/§3.14）。上行 = 自适应高度文本域；工具条左组 =
 * 附件 ＋（v2.59：添加文件或图片，路径引用 v1）/ 权限模式 chip；右组 =
 * 用量环（UsageRing，§3.12）→ 模型 chip（§3.30 选择器：会话内写覆盖、
 * 首页写缺省）→ 发送键。卡片内底行 = 工作区 chip（WorkspacePicker，项目
 * 归属随手切）。收缩规则 = 左组固定不缩、右组可缩（模型 chip 先压字）。
 * Enter 发送 / Shift+Enter 换行（enterToSend 开关）。
 *
 * v4.11 坞内层叠（自下而上）：输入卡 → 审批/ask 卡（§3.17/§3.39① 决策后
 * 即隐、对话流零气泡）→ 排队卡（§3.39②）→ 回到底部钮（§3.39③，
 * ChatTranscript 按 composer 高度定位）。发送键状态机：运行中 + 空草稿 =
 * 停止方块（可点）；运行中 + 有草稿 = 发送箭头（入队）；重放 = 禁用。
 * Esc 链（§3.25/§3.39⑤）：跳过 ask → 拒绝审批 → 停止 → 交出焦点。
 */
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useApprovalsStore } from "../../stores/approvals";
import { useAsksStore } from "../../stores/asks";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useShellStore } from "../../stores/shell";
import { useToastStore } from "../../stores/toast";
import { Icon } from "../../lib/icons";
import AskCard from "./AskCard.vue";
import AttachButton from "./AttachButton.vue";
import ModelSelector from "./ModelSelector.vue";
import PermissionCard from "./PermissionCard.vue";
import PermissionModePicker from "./PermissionModePicker.vue";
import QueueCard from "./QueueCard.vue";
import UsageRing from "./UsageRing.vue";
import WorkspacePicker from "./WorkspacePicker.vue";

const emit = defineEmits<{ height: [h: number] }>();

const { t } = useI18n();
const chat = useChatStore();
const settings = useSettingsStore();
const shell = useShellStore();
const toast = useToastStore();
const approvals = useApprovalsStore();
const asks = useAsksStore();

const draft = ref("");
const ta = ref<HTMLTextAreaElement | null>(null);
const dock = ref<HTMLElement | null>(null);
const showModels = ref(false);
/** §3.25：↑（输入框为空时）召回上一条已发送输入。 */
const lastSent = ref("");

// 模型浮层外点关闭（走查实抓缺陷：选择器是五个弹层里唯一没有外点/Esc
// 收口的——脚本化点击后浮层长挂不落）。锚 = chip-anchor（触发钮 + 浮层
// 同容器，与权限/工作区 chip 的 root.contains 同一语义）。
const modelAnchor = ref<HTMLElement | null>(null);
function onDocPointerDown(e: PointerEvent) {
  if (!showModels.value) return;
  if (!(modelAnchor.value?.contains(e.target as Node) ?? false)) {
    showModels.value = false;
  }
}

/** 坞内决策卡：一次只渲染队首（队列数在卡头如实显示）。 */
const firstApproval = computed(() => approvals.pending[0] ?? null);
const firstAsk = computed(() => asks.pending[0] ?? null);
/** 贴合柱面（§3.11 v4.11 参考图）：有任一坞卡时输入卡顶角改方角——
 * 卡与输入卡同底色零间隙，方角相接才是一根完整面（圆角会在接缝两角
 * 留透空缺口）。 */
const stacked = computed(
  () => !!(firstApproval.value || firstAsk.value || chat.queue.length),
);

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

/**
 * 发送入口统一走 chat.submit（§3.39②：运行中 = 入队，收束后自动派发；
 * /整理 前端拦截也在 store 内）。草稿清空 + 召回记录不变。
 */
function submit() {
  const text = draft.value;
  if (!text.trim()) return;
  draft.value = "";
  lastSent.value = text;
  chat.submit(text);
  void nextTick(autoResize);
}

/** 发送键状态机（§3.11 v4.11）：运行中 + 空草稿 = 停止；其余 = 发送/入队。 */
const stopArmed = computed(() => chat.running && draft.value.trim() === "");
function onSendClick() {
  if (stopArmed.value) {
    chat.stop();
    return;
  }
  submit();
}

function onKeydown(e: KeyboardEvent) {
  // §3.25/§3.39⑤ Esc 链：跳过 ask → 拒绝审批 → 停止 → 交出焦点；
  // 模型浮层开着时 Esc 归它（自持收口），这里不抢。输入法组合中是取消候选。
  if (e.key === "Escape" && !e.isComposing) {
    if (showModels.value) return;
    const ask = asks.pending[0];
    if (ask) {
      void asks.skip(ask.request_id).catch(() => {});
      return;
    }
    const approval = approvals.pending[0];
    if (approval) {
      approvals
        .decide(approval, "deny")
        .catch((err) => toast.push("danger", t("approval.decisionFailed"), String(err)));
      return;
    }
    if (chat.running) {
      chat.stop();
      return;
    }
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
  submit();
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
  document.addEventListener("pointerdown", onDocPointerDown);
  if (dock.value) {
    ro = new ResizeObserver(() => emit("height", dock.value?.offsetHeight ?? 0));
    ro.observe(dock.value);
    emit("height", dock.value.offsetHeight);
  }
});
onBeforeUnmount(() => {
  document.removeEventListener("pointerdown", onDocPointerDown);
  ro?.disconnect();
});
</script>

<template>
  <div ref="dock" class="composer-dock">
    <!-- 坞内卡片层（§3.11 v4.11 自下而上：输入卡 → 审批/ask → 排队卡）；
         决策卡决策后即隐（后端仍留审计与折叠记录行），对话流零气泡 -->
    <div class="dock-cards conv-w">
      <PermissionCard
        v-if="firstApproval"
        :req="firstApproval"
        :queue="approvals.queueCount"
      />
      <AskCard v-if="firstAsk" :req="firstAsk" :queue="asks.queueCount" />
      <QueueCard v-if="chat.queue.length" />
    </div>

    <div
      class="composer conv-w"
      :class="{ 'composer--stacked': stacked }"
      :style="{ pointerEvents: chat.replaying ? 'none' : undefined }"
    >
      <textarea
        ref="ta"
        v-model="draft"
        rows="1"
        class="composer__input scroll-y"
        :placeholder="chat.running ? t('chat.placeholderQueue') : t('chat.placeholder')"
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
          <div ref="modelAnchor" class="chip-anchor">
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
          <!-- 发送键状态机（§3.11 v4.11）：运行中+空草稿 = 停止方块（可点）；
               运行中+有草稿 = 发送箭头（入队）；重放 = 禁用 -->
          <button
            class="send-btn"
            type="button"
            :disabled="chat.replaying || (!chat.running && !draft.trim())"
            :aria-label="
              stopArmed
                ? t('chat.stopGeneration')
                : chat.running
                  ? t('chat.queueAdd')
                  : t('chat.send')
            "
            @click="onSendClick"
          >
            <Icon v-if="stopArmed" name="stop" :size="18" />
            <Icon v-else name="sendUp" :size="18" />
          </button>
        </div>
      </div>
    </div>

    <!-- 工作区 chip 行（§3.11 v3.9，照参考对象补齐）：项目归属随手切，数据
         与侧栏项目段同源；**在卡片外、坐在下方暗影层上**（用户裁定）。
         v2.60 修复批：会话态移到顶栏标题前（纯图标，Topbar 接线）——这里
         只剩首页空态形态 -->
    <div v-if="!chat.items.length" class="composer__wsrow">
      <WorkspacePicker :drop-down="!chat.items.length" />
    </div>
  </div>
</template>

<style scoped>
.composer-dock {
  position: absolute;
  inset-inline: 0;
  bottom: 0;
  padding: 0 var(--lw-gap-24) var(--lw-gap-16);
  z-index: 12;
  pointer-events: none;
}
/* 坞内卡片层（§3.11 v4.11）：三卡与输入卡**贴合堆叠**——用户裁定参考
 * ZCode 原话「卡片是覆盖在对话框上向上延伸的」「类似空白对话框下暗影
 * 像上伸展」：同底色、零间隙，连成一根向上伸展的柱面（卡片本体不再
 * 自带圆角/底色/暗影，由本容器统一持有）。影各元素同款 composer 暗影
 * （兄弟后绘覆盖接缝，视觉 = 整柱一圈暗影）；首卡承顶部圆角；发丝线
 * 只在做卡叠卡时出现。宽度与输入卡同源（conv-w 消费点：宽度规则须落
 * 在本组件——v2.74·fix 实抓缺宽度规则使三卡吃到坞全宽 933 vs 列 724）；
 * min-width:0 防 flex 子项被内容 min-content 顶宽。 */
.dock-cards {
  width: min(100%, var(--lw-content-max));
  margin-inline: auto;
  display: flex;
  flex-direction: column;
  pointer-events: auto;
}
.dock-cards > * {
  min-width: 0;
  background: var(--lw-bg-composer);
  box-shadow: var(--lw-shadow-composer);
}
.dock-cards > :first-child {
  border-radius: var(--lw-r-xl) var(--lw-r-xl) 0 0;
}
.dock-cards > * + * {
  border-top: 1px solid var(--lw-line-subtle);
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
/* 贴合柱面：上方有坞卡时顶角方角化（见 stacked 注），底角保持圆角 */
.composer--stacked {
  border-radius: 0 0 var(--lw-r-xl) var(--lw-r-xl);
}
.composer--stacked .composer__input {
  border-radius: 0;
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
  padding: var(--lw-gap-12) var(--lw-gap-14) var(--lw-gap-4);
  font-size: var(--lw-fs-14);
  line-height: 1.45;
  max-height: 180px;
}
.composer__input::placeholder {
  color: var(--lw-ink-5);
  font-size: var(--lw-fs-12half);
}
.composer__toolbar {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
  padding: var(--lw-gap-10) var(--lw-gap-12) var(--lw-gap-6);
}
/* 工作区 chip 行（§3.11 v3.9 用户裁定）：卡片**外**底行，坐在下方暗影
 * 层上——宽度与卡同源，dock pointer-events:none 需显式放开 */
.composer__wsrow {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
  width: min(100%, var(--lw-content-max));
  margin-inline: auto;
  padding: var(--lw-gap-6) var(--lw-gap-14) 0;
  pointer-events: auto;
}
/* 收缩规则（§3.11 v3.8）：左组固定不缩；右组可缩——窄窗口下先压模型
 * chip 的文字，而不是溢出卡片 */
.composer__left,
.composer__right {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
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
  gap: var(--lw-gap-6);
  min-width: 0;
  height: 28px;
  padding: var(--lw-gap-4) var(--lw-gap-10);
  border-radius: var(--lw-r-full);
  background: var(--lw-wash-2);
  font-size: var(--lw-fs-12half);
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
  /* 禁用态（v2.60 修复批用户裁定「箭头要变成浅色」）：浅色箭头 + 退灰底
   * ——白箭在两主题都落在灰底上可辨（旧 ink 混色在暗主题浅底浅箭不可读） */
  background: color-mix(in oklab, var(--lw-ink-1) 18%, transparent);
  color: var(--lw-send-disabled);
  cursor: default;
}
</style>
