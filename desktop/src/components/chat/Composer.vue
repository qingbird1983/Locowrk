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
import { useChatStore, type AttachmentItem } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useShellStore } from "../../stores/shell";
import { useToastStore } from "../../stores/toast";
import { Icon } from "../../lib/icons";
import { anyLayerOpen } from "../../lib/use-popover";
import type { SlashCommand } from "../../lib/slash-commands";
import type { ContentPartDto } from "../../types";
import AskCard from "./AskCard.vue";
import AttachButton from "./AttachButton.vue";
import ModelSelector from "./ModelSelector.vue";
import PermissionCard from "./PermissionCard.vue";
import PermissionModePicker from "./PermissionModePicker.vue";
import QueueCard from "./QueueCard.vue";
import SlashPanel from "./SlashPanel.vue";
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

// ---- 斜杠命令面板（v2.143，UI shell §3.36① 扩展）：键入 `/` 弹出，
// ↑↓ 选择 / Enter·Tab 或点击直接触发；命令清单与提交拦截同源
// lib/slash-commands.ts。Esc 先关面板再走 §3.25/§3.39⑤ Esc 链。----
const slashPanel = ref<InstanceType<typeof SlashPanel> | null>(null);
const slashDismissed = ref(false);
/** `/` 开头的单 token 草稿（无空白无第二个 `/`）= 面板开着。 */
const slashOpen = computed(
  () => !slashDismissed.value && /^\/[^/\s]*$/.test(draft.value),
);
watch(draft, () => {
  slashDismissed.value = false;
});
function onSlashPick(cmd: SlashCommand) {
  draft.value = "";
  slashDismissed.value = false;
  void chat.submit(`/${cmd.name}`);
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

/** 批 B 推理档位循环钮（供应商稿 §5.3，D3 裁定 = 循环钮）：梯子 = 当前
 * 模型的可发送档位（方言口径，模型选择器数据同一来源）；档位是会话级
 * 属性 → 首页空态隐藏；梯子空（无方言模型）同样隐藏。循环 = [默认,
 * ...梯子]，默认 = 不发档位字段（模型走自家缺省）。 */
const thinkingLadder = computed<string[]>(() => {
  const pid = chat.modelOverride?.providerId || settings.view?.llm.active;
  const model = chat.modelOverride?.model || settings.view?.llm.model || "";
  const p = (settings.view?.llm.providers ?? []).find((x) => x.id === pid);
  return p?.models.find((m) => m.id === model)?.reasoningLadder ?? [];
});
const thinkingCycle = computed(() => [null, ...thinkingLadder.value]);
const thinkingIndex = computed(() =>
  thinkingCycle.value.findIndex((l) => l === chat.thinkingLevel),
);
const inSession = computed(() => chat.sessions.some((x) => x.id === chat.sessionId));
const showThinking = computed(() => inSession.value && thinkingLadder.value.length > 0);
const thinkingLabel = computed(() => chat.thinkingLevel ?? t("model.thinkingDefault"));
function cycleThinking() {
  if (!showThinking.value) return;
  const next = thinkingCycle.value[(thinkingIndex.value + 1) % thinkingCycle.value.length];
  chat.setThinking(next).catch((e) => toast.push("danger", String(e)));
}
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

/** §3.11 v3.9 附件 v1（批 D 收窄）：非图片/PDF 的路径保持 `@路径` 引用
 * 追加进输入框（模型经 fs 工具读取）；图片/PDF 由 AttachButton 直入
 * 附件预览条（上传链路），不再走这里。 */
function onAttach(paths: string[]) {
  if (!paths.length) return;
  const refs = paths.map((p) => `@${p}`).join(" ");
  draft.value = draft.value ? `${draft.value} ${refs}` : refs;
}

// ---- 批 D 多模态：附件预览条 / 粘贴 / 拖拽 / 上传门控（§3.11 v4.24）----
// 附件按钮的图片/PDF 分流在 AttachButton（pick 路径按扩展名路由）；这里
// 只管粘贴/拖拽两条内存文件入口与门控。

function onPaste(e: ClipboardEvent) {
  const files = [...(e.clipboardData?.files ?? [])];
  const media = files.filter(
    (f) => f.type.startsWith("image/") || f.type === "application/pdf",
  );
  if (!media.length) return; // 纯文本粘贴走默认行为
  e.preventDefault();
  for (const f of media) void chat.addAttachmentFile(f);
}

const dragging = ref(false);
let dragDepth = 0;
function onDragEnter(e: DragEvent) {
  if (![...(e.dataTransfer?.types ?? [])].includes("Files")) return;
  dragDepth++;
  dragging.value = true;
}
function onDragLeave() {
  dragDepth = Math.max(0, dragDepth - 1);
  if (dragDepth === 0) dragging.value = false;
}
function onDrop(e: DragEvent) {
  dragDepth = 0;
  dragging.value = false;
  const files = [...(e.dataTransfer?.files ?? [])];
  const media = files.filter(
    (f) => f.type.startsWith("image/") || f.type === "application/pdf",
  );
  for (const f of media) void chat.addAttachmentFile(f);
  if (media.length) e.preventDefault();
}

/** 上传门控（§5.3 第一道）：生效模型图像输入明确不可用（用户钉死 false，
 * 或钉死缺席时目录模态确定不含 image）且预览条里有图片。未知（null 且
 * 非目录来源）放行——目录没有的能力不发明。 */
const imageBlocked = computed(() => {
  const pid = chat.modelOverride?.providerId || settings.view?.llm.active;
  const model = chat.modelOverride?.model || settings.view?.llm.model || "";
  const p = (settings.view?.llm.providers ?? []).find((x) => x.id === pid);
  const m = p?.models.find((x) => x.id === model);
  if (!m) return false;
  if (m.supportsImage === false) return true;
  if (
    m.supportsImage == null &&
    m.sources?.modalities === "catalog" &&
    !m.inputModalities.includes("image")
  ) {
    return true;
  }
  return false;
});
const showImageGate = computed(
  () => imageBlocked.value && chat.attachments.some((a) => a.kind === "image"),
);
function onGateTextOnly() {
  chat.dropImageAttachments();
}
function onGatePickModel() {
  showModels.value = true;
}

function removeAttachment(key: number) {
  chat.removeAttachment(key);
}
function attachmentLabel(a: AttachmentItem): string {
  return a.status === "error"
    ? `${a.name} · ${a.error ?? ""}`
    : a.status === "uploading"
      ? `${a.name} · ${t("mm.uploading")}`
      : a.name;
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
 * `/压缩` 前端拦截也在 store 内，命令名与面板同源 `lib/slash-commands.ts`）。
 * 带附件（批 D）：先收口上传（失败不发、
 * 留条如实展示），parts 随消息发出后预览条由 store 清空；运行中入队不
 * 带附件（排队消息是纯文本——附件等空闲再发，toast 如实提示）。
 */
async function submit() {
  const text = draft.value;
  const hasAttachments = chat.attachments.length > 0;
  if (!text.trim() && !hasAttachments) return;
  if (chat.running) {
    if (hasAttachments) {
      toast.push("warn", t("mm.busyAttach"));
      return;
    }
    draft.value = "";
    lastSent.value = text;
    void chat.submit(text);
    void nextTick(autoResize);
    return;
  }
  let parts: ContentPartDto[] | null = null;
  if (hasAttachments) {
    parts = await chat.settleAttachments();
    if (parts === null) {
      toast.push("danger", t("mm.uploadFailed"));
      return;
    }
  }
  draft.value = "";
  lastSent.value = text;
  if (parts?.length) {
    await chat.send(text, parts);
  } else {
    void chat.submit(text);
  }
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
  // §3.25/§3.39⑤ Esc 链：斜杠面板 → 让位给开着的浮层 → 跳过 ask → 拒绝审批
  // → 停止 → 交出焦点。输入法组合中是取消候选。
  // 「让位」不问具体是哪个浮层（改前只认识 `showModels` 一个，别的浮层一补
  // Esc 就变成"按一次键关一层 + 顺手拒一次审批"= 把不可逆动作放进误触路径）：
  // 浮层开着时它自己会吃掉这一键，优先级判定唯一出处 = `lib/use-popover.ts`。
  if (e.key === "Escape" && !e.isComposing) {
    if (slashOpen.value) {
      slashDismissed.value = true;
      return;
    }
    if (anyLayerOpen()) return;
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
  // 斜杠面板开着：↑↓/Enter/Tab 归面板导航（Enter 不发送、↑ 不召回
  // 上一条——面板开着意味着草稿非空，召回本就不会触发）
  if (slashOpen.value && !e.isComposing && slashPanel.value?.onKey(e)) {
    e.preventDefault();
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
      :class="{ 'composer--stacked': stacked, 'composer--drag': dragging }"
      :style="{ pointerEvents: chat.replaying ? 'none' : undefined }"
      @dragenter="onDragEnter"
      @dragleave="onDragLeave"
      @drop="onDrop"
    >
      <!-- 上传门控提示卡（批 D §9.5 第一道）：当前模型不支持图片 + 有图
           片附件 → 「仅发送文字」剥图 / 「打开模型选择器」 -->
      <div v-if="showImageGate" class="mm-gate" role="alert">
        <Icon name="eye" :size="14" class="mm-gate__icon" aria-hidden="true" />
        <span class="mm-gate__text">{{ t("mm.gateTitle") }}</span>
        <button class="mm-gate__btn" type="button" @click="onGateTextOnly">
          {{ t("mm.gateTextOnly") }}
        </button>
        <button class="mm-gate__btn" type="button" @click="onGatePickModel">
          {{ t("mm.gatePickModel") }}
        </button>
      </div>
      <!-- 附件预览条（批 D §9.2）：有附件才渲染；图片缩略 / PDF 文件 chip
           + 右下 ✕ 移除；上传中/失败态在 title 与边色如实呈现 -->
      <div v-if="chat.attachments.length" class="attach-strip" role="list">
        <div
          v-for="a in chat.attachments"
          :key="a.key"
          class="attach-chip"
          :class="[`is-${a.status}`, { 'is-image': a.kind === 'image' }]"
          role="listitem"
          :title="attachmentLabel(a)"
        >
          <img
            v-if="a.kind === 'image' && a.localUrl"
            :src="a.localUrl"
            class="attach-chip__thumb"
            alt=""
            draggable="false"
          />
          <span v-else class="attach-chip__file">
            <Icon name="paperclip" :size="12" aria-hidden="true" />
            <span class="attach-chip__name">{{ a.name }}</span>
          </span>
          <button
            class="attach-chip__x"
            type="button"
            :aria-label="t('mm.removeAttachment')"
            @click="removeAttachment(a.key)"
          >
            <Icon name="close" :size="10" aria-hidden="true" />
          </button>
        </div>
      </div>
      <!-- 斜杠命令面板（v2.143）：键入 `/` 即弹出，锚在输入卡上方；
           ↑↓ 选择、Enter/Tab 或点击直接触发，Esc 关闭 -->
      <SlashPanel
        v-if="slashOpen"
        ref="slashPanel"
        class="slash-anchor"
        :query="draft.slice(1)"
        @select="onSlashPick"
        @close="slashDismissed = true"
      />
      <textarea
        ref="ta"
        v-model="draft"
        rows="1"
        class="composer__input scroll-y"
        :placeholder="chat.running ? t('chat.placeholderQueue') : t('chat.placeholder')"
        @keydown="onKeydown"
        @paste="onPaste"
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
          <!-- 推理档位循环钮（批 B §5.3 D3 循环钮）：会话级档位循环
               [默认 → ...梯子]，点击下一档、tooltip 当前档 -->
          <button
            v-if="showThinking"
            type="button"
            class="chip chip--thinking"
            :aria-label="t('model.thinkingCycle', { level: thinkingLabel })"
            @click="cycleThinking"
          >
            <Icon name="sparkle" :size="12" class="shrink-0" :class="chat.thinkingLevel ? 'text-accent' : ''" />
            <span class="chip__text">{{ thinkingLabel }}</span>
          </button>
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
              <Icon name="chevronDown" :size="12" class="shrink-0 chip__chevron" />
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
            :disabled="
              chat.replaying ||
              (!chat.running && !draft.trim() && !chat.attachments.length)
            "
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
  /* v4.18：1px 发丝线轮廓（--lw-shadow-composer 的 20px 晕影层退役，
   * 卡面轮廓改由发丝线承载——用户裁定「白雾看得迷糊」） */
  border: 1px solid var(--lw-line);
}
.dock-cards > :first-child {
  border-radius: var(--lw-r-xl) var(--lw-r-xl) 0 0;
}
.dock-cards > * + * {
  /* v4.18：接缝 = 上卡下边与下卡上边 -1px 叠边（恒 1px 不重线），
   * 替代旧 border-top 接缝线——各卡自带全边框后旧法会叠出 2px */
  margin-top: -1px;
}
.composer {
  position: relative;
  width: min(100%, var(--lw-content-max));
  margin-inline: auto;
  border-radius: var(--lw-r-xl);
  background: var(--lw-bg-composer);
  box-shadow: var(--lw-shadow-composer);
  border: 1px solid var(--lw-line);
  pointer-events: auto;
}
/* 贴合柱面：上方有坞卡时顶角方角化（见 stacked 注），底角保持圆角；
 * -1px 外边距把卡列底边与本卡顶边叠成 1px（同 .dock-cards > * + *） */
.composer--stacked {
  margin-top: -1px;
  border-radius: 0 0 var(--lw-r-xl) var(--lw-r-xl);
}
.composer--stacked .composer__input {
  border-radius: 0;
}
/* 拖拽悬停（批 D §9.1）：卡内发丝线变 accent 提示可投放 */
.composer--drag {
  border-color: var(--lw-accent);
}
/* 斜杠命令面板锚位（v2.143）：与输入卡同宽（左右各吃掉 1px 边对齐外沿），
 * 贴输入卡顶边上浮；z 走弹层档（高于 composer 卡、低于模态） */
.slash-anchor {
  position: absolute;
  left: -1px;
  right: -1px;
  bottom: calc(100% + 6px);
  z-index: var(--lw-z-pop);
}
/* 上传门控提示卡（批 D §9.5 第一道）：warn 色警示条 + 两动作 */
.mm-gate {
  display: flex;
  align-items: center;
  gap: var(--lw-gap-8);
  padding: var(--lw-gap-8) var(--lw-gap-14) 0;
  font-size: var(--lw-fs-12half);
  color: var(--lw-warn);
}
.mm-gate__icon {
  flex: 0 0 auto;
}
.mm-gate__text {
  min-width: 0;
  flex: 1 1 auto;
}
.mm-gate__btn {
  flex: 0 0 auto;
  padding: var(--lw-gap-2) var(--lw-gap-8);
  border-radius: var(--lw-r-full);
  font-size: var(--lw-fs-12half);
  color: var(--lw-ink-2);
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.mm-gate__btn:hover {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
/* 附件预览条（批 D §9.2）：输入域上方横向 chip 列 */
.attach-strip {
  display: flex;
  flex-wrap: wrap;
  gap: var(--lw-gap-8);
  padding: var(--lw-gap-10) var(--lw-gap-14) 0;
}
.attach-chip {
  position: relative;
  overflow: visible;
  border: 1px solid var(--lw-line);
  border-radius: var(--lw-r-lg);
  background: var(--lw-bg-elevated);
}
.attach-chip.is-error {
  border-color: var(--lw-danger);
}
.attach-chip.is-uploading {
  opacity: 0.72;
}
.attach-chip__thumb {
  display: block;
  width: 56px;
  height: 56px;
  object-fit: cover;
  /* 内圆角 = 外 chip 圆角收一档（形状阶梯；✕ 钮悬在 chip 外不能裁切） */
  border-radius: var(--lw-r-md);
  user-select: none;
}
.attach-chip__file {
  display: flex;
  max-width: 180px;
  height: 56px;
  align-items: center;
  gap: var(--lw-gap-6);
  padding: 0 var(--lw-gap-12);
  color: var(--lw-ink-2);
}
.attach-chip__name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: var(--lw-fs-12half);
}
.attach-chip__x {
  position: absolute;
  top: -6px;
  right: -6px;
  z-index: 1;
  display: inline-flex;
  width: 18px;
  height: 18px;
  align-items: center;
  justify-content: center;
  border-radius: var(--lw-r-full);
  background: var(--lw-ink-3);
  color: var(--lw-bg);
  transition: background var(--dur-fast) var(--ease-standard);
}
.attach-chip__x:hover {
  background: var(--lw-ink-1);
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
/* 推理档位循环钮（批 B §5.3）：模型 chip 同族小号；有档时字形点亮 */
.chip--thinking {
  cursor: pointer;
  flex: none;
  color: var(--lw-ink-3);
}
.chip--thinking:enabled:hover {
  color: var(--lw-ink-1);
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
  /* 32px（v2.94 目检裁定：36 偏大收一档）；字形 18 = 尺寸阶梯发送键特例
   * （勾⑥），stop 方块字形已重绘放大（icons.ts，占格 35%→50%） */
  width: 32px;
  height: 32px;
  border-radius: var(--lw-r-full);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  /* 底/字互换（v2.94 目检裁定）：accent-ink 底 + accent 字——反转单色钮 */
  background: var(--lw-accent-ink);
  color: var(--lw-accent);
  flex: 0 0 auto;
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard),
    transform var(--dur-instant) var(--ease-standard);
}
.send-btn:hover:not(:disabled) {
  /* 主实心的 accent-hover 是 accent 底专用；本钮底已换 accent-ink，hover
   * 向 accent 混 10% 微移（同族唯一特例，随本裁定生效） */
  background: color-mix(in oklab, var(--lw-accent-ink) 90%, var(--lw-accent));
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
