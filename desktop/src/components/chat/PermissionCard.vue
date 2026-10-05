<script setup lang="ts">
/**
 * 审批决策卡（UI shell doc §3.17 v4.11）——坞卡：渲染在 composer 坞内
 * （Composer.dock-cards 槽位），不再进对话流（§3.39④ 裁定：选择后即隐、
 * 对话流零气泡；审计 = 会话日志 + 过程组内的折叠记录行 §3.7）。
 *
 * 版式按用户参考图（ZCode 实卡）：头行 = ✋ + 「权限请求：动作名」粗体
 * （§3.18 中文名，原生 tooltip 留工具 id）+ 排队数/子 Agent 徽章 + 右侧
 * 动作 = [拒绝 Esc] ghost + [允许一次 | ⌄] 分体主钮（chevron 上弹「允许
 * 本次对话」，主路径仍是最小授权）；下接参数预览（裸行 mono 单行省略，
 * 完整参数在转录工具行里 hover 复制）+ 元信息行（工具自述 + 环境归属
 * 11px）。风险档不在坞卡重复表达（转录工具行副作用徽章可见）。硬门
 * （越界/隐私）提示行保留；硬门卡只有「拒绝 / 允许一次」两键。守夜倒
 * 计时 = 卡底 2px 细条，0 = 不限不渲染。决策后焦点回 composer。
 */
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useApprovalsStore, type ApprovalDecisionKind } from "../../stores/approvals";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { Icon } from "../../lib/icons";
import { toolAction, toolArgSummary } from "../../lib/tool-summary";
import type { ApprovalRequest } from "../../types";

const props = defineProps<{
  req: ApprovalRequest;
  /** 排队数（还有 N 个待处理 = 除本卡之外的待决数）。 */
  queue: number;
}>();

const { t } = useI18n();
const approvals = useApprovalsStore();
const chat = useChatStore();
const settings = useSettingsStore();

/** ⑮ 子 Agent 派生的调用（session_id 非当前会话）打标识。 */
const isSubagent = computed(
  () => !!props.req.session_id && props.req.session_id !== chat.sessionId,
);

/** 提问行主词 = §3.18 中文动作名（表外回落工具 id）。 */
const actionName = computed(() => {
  const a = toolAction(props.req.tool);
  return a ? t(a.key, a.params ?? {}) : props.req.tool;
});

/** 参数预览（§3.17 v4.11）：单行省略 mono——加工摘要与工具行同源。 */
const argPreview = computed(() =>
  toolArgSummary(
    props.req.tool,
    props.req.args,
    [settings.view?.dataDir ?? "", ...(settings.view?.tools.fsRoots ?? [])].filter(Boolean),
  ),
);

/** ㉘·B 越界门 / ㉘·C 隐私门：硬门卡逐次审批、只许一次——不渲染「允许
 * 本次对话」（模式放行与豁免都跳不过硬门）。 */
const isExternal = computed(() => props.req.external === true);
/** 隐私门类别键 → i18n 文案（未知键裸显兜底）。 */
const sensitiveLabel = computed(() => {
  const key = props.req.sensitive;
  if (!key) return "";
  const path = `approval.sensitive.${key}`;
  return t(path) === path ? key : t(path);
});
const isHardGate = computed(() => isExternal.value || sensitiveLabel.value !== "");

/** 环境行（§3.17⑤，㉘·B 如实三级回落）：载荷 workspace（会话真实归属）
 * → fsRoots 首项 → 临时会话。 */
const workspace = computed(
  () => props.req.workspace || settings.view?.tools.fsRoots?.[0] || "",
);

/** 「允许本次对话」下拉（v4.11 收进 chevron：主路径 = 允许一次，最小
 * 授权是缺省；会话豁免是显式升级动作）。 */
const showScope = ref(false);
function onDocPointerDown(e: PointerEvent) {
  if (!showScope.value) return;
  if (!(scopeAnchor.value?.contains(e.target as Node) ?? false)) {
    showScope.value = false;
  }
}
const scopeAnchor = ref<HTMLElement | null>(null);
onMounted(() => document.addEventListener("pointerdown", onDocPointerDown));
onBeforeUnmount(() => document.removeEventListener("pointerdown", onDocPointerDown));

/** 守夜倒计时只做显示（真执行者是后端 fail-closed）；0 = 不限不渲染。 */
const limit = computed(() => props.req.timeout_secs ?? 0);
const secondsLeft = ref(limit.value);
let timer: number | null = null;
onMounted(() => {
  if (limit.value <= 0) return;
  timer = window.setInterval(() => {
    if (secondsLeft.value > 0) secondsLeft.value--;
    if (secondsLeft.value <= 0 && timer !== null) {
      window.clearInterval(timer);
      timer = null;
    }
  }, 1000);
});
onBeforeUnmount(() => {
  if (timer !== null) window.clearInterval(timer);
});
const timeoutPct = computed(() =>
  limit.value > 0 ? Math.max(0, secondsLeft.value / limit.value) * 100 : 0,
);

// 决策中防重；命令报错 → 内联报错 + 恢复可点
const deciding = ref(false);
const errorText = ref("");
async function decide(kind: ApprovalDecisionKind) {
  if (deciding.value) return;
  deciding.value = true;
  errorText.value = "";
  showScope.value = false;
  try {
    await approvals.decide(props.req, kind);
  } catch (err) {
    errorText.value = t("approval.decisionFailed", { reason: String(err) });
  } finally {
    deciding.value = false;
  }
}
</script>

<template>
  <section class="approval-card" role="region" aria-label="需要权限">
    <!-- ① 头行：✋ + 权限请求：动作名 + 徽章 + 动作右置（参考图版式） -->
    <div class="approval-card__head">
      <Icon name="hand" :size="14" class="approval-card__icon" />
      <span class="approval-card__title" :title="req.tool">
        {{ t("approval.cardTitle") }}：{{ actionName }}
      </span>
      <span v-if="queue > 0" class="approval-card__queue">
        {{ t("approval.queue", { n: queue }) }}
      </span>
      <span v-if="isSubagent" class="approval-card__sub">
        {{ t("approval.subagentTag") }}
      </span>

      <div class="approval-card__acts">
        <button
          class="approval-card__btn approval-card__btn--ghost"
          type="button"
          :disabled="deciding"
          @click="decide('deny')"
        >
          {{ t("approval.deny") }}
          <span class="approval-card__keycap">Esc</span>
        </button>
        <!-- 分体主钮（参考图）：主段 = 允许一次（最小授权缺省），chevron
             段上弹「允许本次对话」；硬门卡收窄成单键（围栏外只许一次） -->
        <div v-if="!isHardGate" ref="scopeAnchor" class="approval-card__split">
          <button
            class="approval-card__btn approval-card__btn--primary approval-card__split-main"
            type="button"
            :disabled="deciding"
            @click="decide('once')"
          >
            {{ t("approval.allowOnce") }}
          </button>
          <button
            class="approval-card__split-chev"
            type="button"
            :aria-expanded="showScope"
            :aria-label="t('approval.allowSession')"
            :disabled="deciding"
            @click="showScope = !showScope"
          >
            <Icon name="chevronDown" :size="12" />
          </button>
          <!-- 「允许本次对话」上弹单项：进退场由 Transition 接管（标准 §二①） -->
          <Transition name="lw-pop-up">
            <button
              v-if="showScope"
              class="approval-card__scope-item"
              type="button"
              @click="decide('session')"
            >
              {{ t("approval.allowSession") }}
            </button>
          </Transition>
        </div>
        <button
          v-else
          class="approval-card__btn approval-card__btn--primary"
          type="button"
          :disabled="deciding"
          @click="decide('once')"
        >
          {{ t("approval.allowOnce") }}
        </button>
      </div>
    </div>

    <!-- ② 参数预览：单行省略 mono（完整参数在转录工具行里） -->
    <div class="approval-card__preview" :title="argPreview">{{ argPreview }}</div>

    <!-- ③ 元信息行：工具自述 + 环境归属（11px） -->
    <p class="approval-card__meta">
      <span v-if="req.description" class="approval-card__desc">{{ req.description }}</span>
      <span class="approval-card__env" :title="workspace || undefined">
        {{
          workspace
            ? t("approval.envProject", { path: workspace })
            : t("approval.envTemp")
        }}
      </span>
    </p>

    <!-- ㉘·C 硬门提示行（越界 / 隐私，warn 色） -->
    <p v-if="isExternal" class="approval-card__hardgate">
      {{ t("approval.externalHint") }}
    </p>
    <p v-if="sensitiveLabel" class="approval-card__hardgate">
      {{ t("approval.sensitiveLead", { kind: sensitiveLabel }) }}
    </p>
    <p v-if="errorText" class="approval-card__error">{{ errorText }}</p>

    <!-- 守夜倒计时（§3.17 v4.11：卡底 2px 细条；0 = 不限不渲染） -->
    <div
      v-if="limit > 0"
      class="approval-card__timeout"
      role="timer"
      :title="t('approval.timeoutHint', { n: secondsLeft })"
    >
      <div class="approval-card__timeout-bar" :style="{ width: timeoutPct + '%' }" />
    </div>
  </section>
</template>

<style scoped>
/* 卡面（底色/圆角/暗影）由 Composer.dock-cards 容器统一持有（贴合柱面
 * 语义），本组件只管内容版式。 */
.approval-card {
  position: relative;
  padding: 10px 14px 11px;
  /* 不设 overflow:hidden——「允许本次对话」上弹单项要冒出卡顶；
   * 守夜 2px 细条在卡界内（bottom:0），无需裁切 */
}

.approval-card__head {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}
.approval-card__icon {
  flex: 0 0 auto;
  color: var(--lw-ink-2);
}
.approval-card__title {
  min-width: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--lw-ink-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.approval-card__queue {
  flex: 0 0 auto;
  font-size: 10.5px;
  color: var(--lw-ink-3);
}
.approval-card__sub {
  flex: 0 0 auto;
  font-size: 10.5px;
  padding: 1px 8px;
  border-radius: var(--lw-r-full);
  background: color-mix(in oklab, var(--lw-purple) 16%, transparent);
  color: var(--lw-purple);
}

.approval-card__acts {
  margin-left: auto;
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  gap: 6px;
}
.approval-card__btn {
  height: var(--lw-ctl);
  padding: 0 12px;
  border-radius: var(--lw-r-md);
  font-size: 12px;
  font-weight: 500;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.approval-card__btn:disabled {
  opacity: 0.4;
  cursor: default;
}
.approval-card__btn--ghost {
  color: var(--lw-ink-2);
}
.approval-card__btn--ghost:hover:not(:disabled) {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.approval-card__keycap {
  font-size: 9.5px;
  padding: 0 4px;
  border-radius: var(--lw-r-3xs);
  background: var(--lw-wash-3);
  color: var(--lw-ink-3);
}
.approval-card__btn--primary {
  background: var(--lw-accent);
  color: var(--lw-accent-ink);
  font-weight: 600;
}
.approval-card__btn--primary:hover:not(:disabled) {
  background: var(--lw-accent-hover);
}

/* 分体主钮（参考图）：主段圆左、chevron 段圆右，同底色中缝发丝线 */
.approval-card__split {
  position: relative;
  display: inline-flex;
  align-items: center;
}
.approval-card__split-main {
  border-radius: var(--lw-r-md) 0 0 var(--lw-r-md);
}
.approval-card__split-chev {
  height: var(--lw-ctl);
  padding: 0 6px;
  border-radius: 0 var(--lw-r-md) var(--lw-r-md) 0;
  background: var(--lw-accent);
  color: var(--lw-accent-ink);
  border-left: 1px solid color-mix(in oklab, var(--lw-accent-ink) 25%, transparent);
  display: inline-flex;
  align-items: center;
  transition: background var(--dur-fast) var(--ease-standard);
}
.approval-card__split-chev:hover:not(:disabled) {
  background: var(--lw-accent-hover);
}
.approval-card__split-chev:disabled {
  opacity: 0.4;
}

/* 「允许本次对话」上弹单项（锚在分体钮上）：浮层唯一式 = elevated 面 +
 * 1px line 围边 + 微弱悬浮影 + 圆角 14（标准 §二③） */
.approval-card__scope-item {
  position: absolute;
  right: 0;
  bottom: calc(100% + 6px);
  white-space: nowrap;
  height: 30px;
  padding: 0 12px;
  border-radius: var(--lw-r-md-plus);
  border: 1px solid var(--lw-line);
  background: var(--lw-bg-elevated);
  box-shadow: var(--lw-shadow-pop);
  font-size: 12px;
  color: var(--lw-ink-1);
  z-index: 1;
}
.approval-card__scope-item:hover {
  background: var(--lw-wash-2);
}

/* 参数预览（参考图）：裸行 mono 单行省略（无底色盒），完整参数在转录
 * 工具行里 */
.approval-card__preview {
  margin-top: 7px;
  font-family: var(--font-mono);
  font-size: 11.5px;
  line-height: 1.5;
  color: var(--lw-ink-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.approval-card__meta {
  margin-top: 6px;
  display: flex;
  align-items: baseline;
  gap: 10px;
  min-width: 0;
}
.approval-card__desc {
  flex: 1 1 auto;
  min-width: 0;
  font-size: 11px;
  line-height: 1.45;
  color: var(--lw-ink-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.approval-card__env {
  flex: 0 1 auto;
  min-width: 0;
  font-size: 11px;
  color: var(--lw-ink-4);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  direction: rtl;
  text-align: left;
}

.approval-card__hardgate {
  margin-top: 4px;
  font-size: 11.5px;
  font-weight: 500;
  line-height: 1.45;
  color: var(--lw-warn);
}

.approval-card__error {
  margin-top: 6px;
  font-size: 12px;
  color: var(--lw-danger);
}

.approval-card__timeout {
  position: absolute;
  inset: auto 0 0 0;
  height: 2px;
}
.approval-card__timeout-bar {
  height: 100%;
  background: color-mix(in oklab, var(--lw-ink-3) 45%, transparent);
}
</style>
