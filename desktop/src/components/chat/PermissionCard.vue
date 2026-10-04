<script setup lang="ts">
/**
 * 审批决策卡（UI shell doc §3.17，㉕·对话区①）——审批不是弹窗：转录流里的
 * 内联决策卡，紧跟触发它的工具行渲染（ChatTranscript 注入）。用户能继续读
 * 上下文、能滚动；"必须决定才能继续"由内核的轮次阻塞保证，不由遮罩保证。
 *
 * 六段（照实机）：头行（需要权限 + 排队数 + 子 Agent 标识 + 风险胶囊）/
 * 提问行（中文动作名，§3.18）/ 理由行（工具自述）/
 * 参数区（顶层键值行，bg-inset、max-h 240 滚动、右上复制）/
 * 环境行（会话真实归属 → fsRoots[0] → 临时会话——批准前必须知道在哪跑）/
 * 动作行（超时秒数文案 + 三键；外部路径卡两键）。风险靠染底 + 胶囊表达（平面内无边框）；
 * 主按钮 = 允许一次（最小授权是默认路径）。决策成功后焦点回 composer。
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useApprovalsStore, type ApprovalDecisionKind } from "../../stores/approvals";
import { useChatStore } from "../../stores/chat";
import { useSettingsStore } from "../../stores/settings";
import { useShellStore } from "../../stores/shell";
import { Icon } from "../../lib/icons";
import { toolAction } from "../../lib/tool-summary";
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
const shell = useShellStore();

/** 风险三档（§3.17）：副作用映射，缺字段按高（宁严勿松，与「side_effect
 * 缺失按 Irreversible」同一纪律）。 */
const risk = computed<"high" | "medium" | "low">(() => {
  if (props.req.side_effect === "compensable") return "medium";
  if (props.req.side_effect === "pure") return "low";
  return "high";
});
const riskLabel = computed(
  () =>
    ({
      high: t("approval.riskHigh"),
      medium: t("approval.riskMedium"),
      low: t("approval.riskLow"),
    })[risk.value],
);

/** ⑮ 子 Agent 派生的调用（session_id 非当前会话）打标识。 */
const isSubagent = computed(
  () => !!props.req.session_id && props.req.session_id !== chat.sessionId,
);

/** 提问行主词 = §3.18 中文动作名（表外回落工具 id）。 */
const actionName = computed(() => {
  const a = toolAction(props.req.tool);
  return a ? t(a.key, a.params ?? {}) : props.req.tool;
});

/** 参数区（§3.17④）：顶层键值行，命令类值 $ 前缀——不裸贴 JSON。 */
const argLines = computed(() => {
  const args = props.req.args;
  if (!args || typeof args !== "object") {
    return [{ flag: "", value: String(args ?? ""), cmd: false }];
  }
  return Object.entries(args as Record<string, unknown>).map(([k, v]) => ({
    flag: k,
    value: typeof v === "string" ? v : JSON.stringify(v),
    cmd: k === "command" || k === "cmd",
  }));
});

async function copyArgs() {
  try {
    await navigator.clipboard.writeText(JSON.stringify(props.req.args, null, 2));
  } catch {
    /* 剪贴板不可用不打断 */
  }
}

/** 环境行（§3.17⑤，㉘·B 如实三级回落）：载荷 workspace（会话真实归属）
 * → fsRoots 首项 → 临时会话。 */
const workspace = computed(
  () => props.req.workspace || settings.view?.tools.fsRoots?.[0] || "",
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

/** 超时倒计时只做显示（§3.17：无进度条动画）——真执行者是后端守夜任务
 * （fail-closed）。秒数读载荷 timeout_secs（v2.49 可配置原则：阈值在
 * 设置页 tools.approvalTimeoutSecs，0 = 不限 → 不渲染倒计时行）。 */
const limit = computed(() => props.req.timeout_secs ?? 100);
const secondsLeft = ref(limit.value);
let timer: number | null = null;
watch(limit, (v) => {
  secondsLeft.value = v;
});
onMounted(() => {
  if (limit.value <= 0) return; // 不限：不起倒计时
  timer = window.setInterval(() => {
    if (secondsLeft.value > 0) secondsLeft.value--;
    if (secondsLeft.value === 0 && timer !== null) {
      window.clearInterval(timer);
      timer = null;
    }
  }, 1000);
});
onBeforeUnmount(() => {
  if (timer !== null) window.clearInterval(timer);
});
const timeoutText = computed(() =>
  secondsLeft.value > 0
    ? t("approval.timeoutHint", { n: secondsLeft.value })
    : t("approval.timeoutNow"),
);

// 决策中防重：三键同时置 disabled；命令报错 → 内联报错 + 恢复可点
const deciding = ref(false);
const errorText = ref("");
async function decide(kind: ApprovalDecisionKind) {
  if (deciding.value) return;
  deciding.value = true;
  errorText.value = "";
  try {
    await approvals.decide(props.req, kind);
    // §3.17：决策后焦点回 composer——批完立刻能接着打字
    shell.composerFocusTick++;
  } catch (err) {
    errorText.value = t("approval.decisionFailed", { reason: String(err) });
  } finally {
    deciding.value = false;
  }
}
</script>

<template>
  <section class="permission" role="region" aria-label="需要权限" :data-risk="risk">
    <!-- ① 头行 -->
    <div class="permission__head">
      <span class="permission__title">{{ t("approval.cardTitle") }}</span>
      <span v-if="queue > 0" class="permission__queue">
        {{ t("approval.queue", { n: queue }) }}
      </span>
      <span v-if="isSubagent" class="permission__sub">
        {{ t("approval.subagentTag") }}
      </span>
      <span class="permission__pill">{{ riskLabel }}</span>
    </div>

    <!-- ② 提问行 / ③ 理由行 + ㉘·C 硬门提示行（越界 / 隐私，warn 色） -->
    <p class="permission__ask">
      {{ t("approval.askLead") }}<b>{{ actionName }}</b>{{ t("approval.askTail") }}
    </p>
    <p v-if="isExternal" class="permission__hardgate">
      {{ t("approval.externalHint") }}
    </p>
    <p v-if="sensitiveLabel" class="permission__hardgate">
      {{ t("approval.sensitiveLead", { kind: sensitiveLabel }) }}
    </p>
    <p v-if="req.description" class="permission__reason">{{ req.description }}</p>

    <!-- ④ 参数区 -->
    <div class="permission__args">
      <div class="permission__args-head">
        <span class="permission__label">{{ t("approval.inputLabel") }}</span>
        <button
          class="copy-btn"
          type="button"
          :title="t('approval.inputCopy')"
          :aria-label="t('approval.inputCopy')"
          @click="copyArgs"
        >
          <Icon name="copy" :size="12" />
        </button>
      </div>
      <div class="permission__well">
        <div v-for="(l, i) in argLines" :key="i" class="permission__line">
          <span v-if="l.flag" class="permission__flag">{{ l.flag }}</span
          >{{ l.cmd ? "$ " : "" }}{{ l.value }}
        </div>
      </div>
    </div>

    <!-- ⑤ 环境行 -->
    <p class="permission__meta" :title="workspace || undefined">
      {{
        workspace
          ? t("approval.envProject", { path: workspace })
          : t("approval.envTemp")
      }}
    </p>

    <!-- ⑥ 动作行：主按钮 = 允许一次（最小授权是默认路径）；外部路径卡
         只有「拒绝 / 允许一次」两键（PI 借鉴：围栏外只许一次） -->
    <div class="permission__acts">
      <span v-if="limit > 0" class="permission__timeout">{{ timeoutText }}</span>
      <div class="permission__btns">
        <button
          class="permission__btn permission__btn--ghost"
          type="button"
          :disabled="deciding"
          @click="decide('deny')"
        >
          {{ t("approval.deny") }}
        </button>
        <button
          v-if="!isHardGate"
          class="permission__btn permission__btn--secondary"
          type="button"
          :disabled="deciding"
          @click="decide('session')"
        >
          {{ t("approval.allowSession") }}
        </button>
        <button
          class="permission__btn permission__btn--primary"
          type="button"
          :disabled="deciding"
          @click="decide('once')"
        >
          {{ t("approval.allowOnce") }}
        </button>
      </div>
    </div>
    <p v-if="errorText" class="permission__error">{{ errorText }}</p>
  </section>
</template>

<style scoped>
.permission {
  margin: 10px 0;
  padding: 14px 16px;
  border-radius: var(--lw-r-md);
  /* 风险靠染底表达（§6.1 平面内无边框纪律） */
  background: color-mix(in oklab, var(--lw-danger) 7%, transparent);
}
.permission[data-risk="medium"] {
  background: color-mix(in oklab, var(--lw-warn) 7%, transparent);
}
.permission[data-risk="low"] {
  background: var(--lw-wash-1);
}

.permission__head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.permission__title {
  font-size: 13px;
  font-weight: 600;
  color: var(--lw-ink-1);
}
.permission__queue {
  font-size: 10.5px;
  color: var(--lw-ink-3);
}
.permission__sub {
  font-size: 10.5px;
  padding: 1px 8px;
  border-radius: var(--lw-r-full);
  background: color-mix(in oklab, var(--lw-purple) 16%, transparent);
  color: var(--lw-purple);
}
.permission__pill {
  margin-left: auto;
  font-size: 10.5px;
  font-weight: 500;
  padding: 1px 8px;
  border-radius: var(--lw-r-full);
  background: color-mix(in oklab, var(--lw-danger) 14%, transparent);
  color: var(--lw-danger);
}
.permission[data-risk="medium"] .permission__pill {
  background: color-mix(in oklab, var(--lw-warn) 14%, transparent);
  color: var(--lw-warn);
}
.permission[data-risk="low"] .permission__pill {
  background: var(--lw-wash-3);
  color: var(--lw-ink-2);
}

.permission__ask {
  margin-top: 10px;
  font-size: 13px;
  line-height: 1.45;
  color: var(--lw-ink-2);
}
.permission__ask b {
  font-weight: 600;
  color: var(--lw-ink-1);
}
.permission__reason {
  margin-top: 2px;
  font-size: 12px;
  line-height: 1.45;
  color: var(--lw-ink-3);
}

/* ㉘·C 硬门提示行（越界 / 隐私）：warn 色小字，压在理由行上方 */
.permission__hardgate {
  margin-top: 4px;
  font-size: 12px;
  font-weight: 500;
  line-height: 1.45;
  color: var(--lw-warn);
}

.permission__args {
  margin-top: 10px;
}
.permission__args-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 4px;
}
.permission__label {
  font-size: 10.5px;
  letter-spacing: 0.02em;
  color: var(--lw-ink-4);
}
.permission__well {
  background: var(--lw-bg-inset);
  border-radius: 10px;
  padding: 8px 10px;
  max-height: 240px;
  overflow-y: auto;
  font-family: var(--font-mono);
  font-size: 11.5px;
  line-height: 1.55;
  color: var(--lw-ink-2);
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.permission__flag {
  display: inline-block;
  min-width: 7ch;
  margin-right: 1ch;
  color: var(--lw-ink-3);
}

.permission__meta {
  margin-top: 8px;
  font-size: 12px;
  color: var(--lw-ink-3);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.permission__acts {
  margin-top: 10px;
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}
.permission__timeout {
  margin-right: auto;
  font-size: 10.5px;
  color: var(--lw-ink-4);
  font-variant-numeric: tabular-nums;
}
.permission__btns {
  display: flex;
  align-items: center;
  gap: 8px;
}
.permission__btn {
  height: var(--lw-ctl);
  padding: 0 12px;
  border-radius: var(--lw-r-md);
  font-size: 12px;
  font-weight: 500;
  transition: background var(--dur-fast) var(--ease-standard),
    color var(--dur-fast) var(--ease-standard);
}
.permission__btn:disabled {
  opacity: 0.4;
  cursor: default;
}
.permission__btn--ghost {
  color: var(--lw-ink-2);
}
.permission__btn--ghost:hover:not(:disabled) {
  background: var(--lw-wash-2);
  color: var(--lw-ink-1);
}
.permission__btn--secondary {
  background: var(--lw-wash-3);
  color: var(--lw-ink-1);
}
.permission__btn--secondary:hover:not(:disabled) {
  background: color-mix(in oklab, var(--lw-ink-1) 10%, transparent);
}
.permission__btn--primary {
  background: var(--lw-accent);
  color: var(--lw-accent-ink);
  font-weight: 600;
}
.permission__btn--primary:hover:not(:disabled) {
  background: var(--lw-accent-hover);
}

.permission__error {
  margin-top: 8px;
  font-size: 12px;
  color: var(--lw-danger);
}
</style>
