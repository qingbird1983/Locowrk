// stores/chat/notices.ts —— 系统通知条的生产口 + M0 观测告警（T22 刀二搬出）。
// S4 刀② 记忆面退役后「整理发丝线」这个生产口删除，只剩通用条 / 压缩发丝线 / 原文条。
// 这一族只做一件事：**往 items 里落「UI 陈述」**（SystemNotice 模式），不发请求、不碰后端。
// 参数名一律 `st`（不是 `s`）：原实现内部的 `.find((s) => …)` 用的是 `s`，
// 把 store 状态参数也叫 `s` 会形成遮蔽 ⇒ 等价比对时看不清哪层是谁。
import { i18n } from "../../i18n";
import { useSettingsStore } from "../settings";
import { useToastStore } from "../toast";
import type { SessionUsage } from "./types";
import { nextId } from "./ids";
import type { ChatState } from "./state";

/** 系统消息入列（SystemNotice 模式的唯一生产入口；首个生产者 ⑨
 * 压缩通知，随后 ⑪ "已记住"）。 */
export function pushNotice(
  st: ChatState,
  key: string,
  params?: Record<string, unknown>,
  actions?: string[],
) {
  st.items.push({ id: nextId(), kind: "system", key, params, actions });
}

/** 发丝线族的**唯一实现**（v2.143 用户裁定「两卡片改一根发丝线，状态居中盖在线上」）：
 * 发起/收束/熔断共用同一根线——在途线被收束事件**就地改写**（一条线从「压缩中」变
 * 「已压缩」），无在途线（自动压缩）直接落收束线。
 *
 * S2 刀⑦ 把两枚常数（`tag` 与"在途那条的 key"）提成参数：`/压缩` 要的是同一根线，
 * 复制一份实现就是"同一属性两个出处"（改写规则、落点、`tone` 三件都会各自漂）。
 * 两个入口各自保留原名，只把参数填进去。 */
/** 收走在途线（发起失败时用——线是「正在做」的陈述，失败后不再成立；就地摘除，
 * 错误由 pushRawSystem 另行陈述）。 */
function dropRunningLine(st: ChatState, tag: string, runningKey: string) {
  for (let i = st.items.length - 1; i >= 0; i--) {
    const it = st.items[i]!;
    if (it.kind === "system" && it.tag === tag && it.key === runningKey) {
      st.items.splice(i, 1);
      return;
    }
  }
}

function pushLine(
  st: ChatState,
  tag: string,
  key: string,
  params?: Record<string, unknown>,
  actions?: string[],
) {
  st.items.push({
    id: nextId(),
    kind: "system",
    key,
    params,
    actions,
    tone: "hairline",
    tag,
  });
}

function rewriteOrPushLine(
  st: ChatState,
  tag: string,
  runningKey: string,
  key: string,
  params?: Record<string, unknown>,
  actions?: string[],
) {
  const items = st.items;
  for (let i = items.length - 1; i >= 0; i--) {
    const it = items[i]!;
    if (it.kind === "system" && it.tag === tag && it.key === runningKey) {
      it.key = key;
      it.params = params;
      it.actions = actions;
      return;
    }
  }
  pushLine(st, tag, key, params, actions);
}

/** 压缩发丝线（tag="compact"，S2 刀⑦ `/压缩`）：发起落「正在压缩」，收束就地改写
 * 成结果线（一次摘要往返要几秒，人得知道按钮生效了）。 */
export function pushCompactNotice(
  st: ChatState,
  key: string,
  params?: Record<string, unknown>,
  actions?: string[],
) {
  rewriteOrPushLine(st, "compact", "notice.compactRunning", key, params, actions);
}

export function dropCompactRunning(st: ChatState) {
  dropRunningLine(st, "compact", "notice.compactRunning");
}

/** 后端判据名 → 语言包那条 key（S2 刀⑦ `/压缩`）。取值域与 Rust 侧同源，由
 * `check-single-source` 的 SS24 钉住：那边新增一条 stop 而这边没补文案，闸就红。
 *
 * ⚠ 兜底那条不许省：认不出的 reason 也要如实说"没压成"并把判据名原样带出来，
 * 绝不许悄悄显示成"已压缩"——那正是本项目一路在反的假告。
 *
 * **这一份住在 notices.ts 而不是 send.ts**（specs/12 刀②）：自动压缩那条路
 * （`agent://compact-status` 的 `failed` 相位）也要按判据名取文案，两条路共用
 * 同一张表 ⇒ 一份事实一个出处。搬过来之前它在 send.ts 里，而 send.ts 是"发东西
 * 的那一族"——判据名表放那儿等于让"读面"去读"写面"的私有物。 */
const COMPACT_SKIP_KEYS: Record<string, string> = {
  "unknown-window": "notice.compactUnknownWindow",
  "too-few-rounds": "notice.compactTooFewRounds",
  "nothing-to-summarize": "notice.compactNothingToSummarize",
  "summary-call": "notice.compactCallFailed",
  overflow: "notice.compactOverflow",
  rejected: "notice.compactRejected",
  // 手动通路理论上到不了这一格（后端 run_manual 不看那枚开关）；留着是为了
  // "真到了也如实说"，而不是让它掉进兜底那条。
  off: "notice.compactSwitchOff",
  // specs/12 刀①：摘要调用超出了总时长预算（辅助路径的硬上限）。与
  // `summary-call` 分开：那是"没答上"，这一条是"没等到"。
  budget: "notice.compactBudget",
  // specs/12 刀①：用户在压缩期间按了「停止生成」——不是失败，是被叫停。
  stopped: "notice.compactStopped",
};

/** 判据名 → 该说哪句话（认不出的走兜底那条，并把名字原样带出）。 */
export function compactSkip(reason: string | null): {
  key: string;
  params?: Record<string, unknown>;
} {
  const known = reason ? COMPACT_SKIP_KEYS[reason] : undefined;
  if (known) return { key: known };
  return { key: "notice.compactUnknown", params: { reason: reason ?? "unknown" } };
}

/** 自动压缩发丝线的 tag（specs/12 刀②）。
 *
 * 为什么与手动 `/压缩` 那根（tag="compact"）分两根：**它们可能同时在场**——
 * 人按 `/压缩` 的那一刻，这一轮还在跑（手动命令要等会话锁），于是手动那根
 * "正在压缩"与自动那根"正在压缩"会同时存在。共用一个 tag 时，自动路径的
 * 收束会把手动那根改写掉 ⇒ 手动那一下的失败提示凭空消失。
 *
 * 两根线的**形态**相同（都是 hairline 发丝线，v2.143 那一根）——同类只许一种
 * 形态说的是形态，不是"一根还是两根"。 */
const COMPACT_AUTO_TAG = "compact-auto";

/** 自动压缩那根线：**改写最后一根同 tag 的线**（没有就落一根新的）。
 *
 * 为什么按 tag 改写、而不是按 key（`notice.compactRunning`）改写：自动压缩在
 * **每个 step 边界**都会再问一次要不要压 ⇒ 一轮里可能连着失败几次。按 key 改写
 * 的话，第二趟的「正在压缩」会另起一根线 ⇒ 一轮攒下 N 根同义的线（用户看到
 * 的是一串"正在压缩 / 没压成 / 正在压缩 / 没压成"）。按 tag 改写则全程只有
 * 一根，最后那一态就是真相。
 */
export function setCompactAutoLine(
  st: ChatState,
  key: string,
  params?: Record<string, unknown>,
) {
  for (let i = st.items.length - 1; i >= 0; i--) {
    const it = st.items[i]!;
    if (it.kind === "system" && it.tag === COMPACT_AUTO_TAG) {
      it.key = key;
      it.params = params;
      it.actions = undefined;
      return;
    }
  }
  pushLine(st, COMPACT_AUTO_TAG, key, params);
}

/** 收走自动压缩那根线（压成了 ⇒ 断层条接手说"省了多少"，这根线不留着）。 */
export function dropCompactAutoLine(st: ChatState) {
  for (let i = st.items.length - 1; i >= 0; i--) {
    const it = st.items[i]!;
    if (it.kind === "system" && it.tag === COMPACT_AUTO_TAG) {
      st.items.splice(i, 1);
      return;
    }
  }
}

/** 原文系统条（key = "__raw__"，ChatMessages 按 params.text 渲染）：
 * 用于"查看详情"展开压缩摘要等内容型陈述。 */
export function pushRawSystem(st: ChatState, text: string) {
  st.items.push({
    id: nextId(),
    kind: "system",
    key: "__raw__",
    params: { text },
  });
}

/** M0 观测告警（KV v2 §7.4，开发者开关缺省关）：会话累计命中率跌破
 * 阈值时 toast 一次（每会话至多一次）。≥3 步才判——冷启动首请求无
 * 缓存可命不是断裂；供应商从未报告过缓存字段（cacheRead/cacheWrite
 * 全 0）不判——未报告 ≠ 命中率塌方，宁缺勿假。 */
export function checkCacheAlert(st: ChatState, su: SessionUsage) {
  if (st.cacheAlertFired || st.replaying) return;
  const dev = useSettingsStore().view?.developer;
  if (!dev?.cacheHitAlert) return;
  if (su.steps < 3 || su.input <= 0) return;
  if (su.cacheRead + su.cacheWrite <= 0) return;
  if (su.cacheRead / su.input >= dev.cacheHitAlertPercent / 100) return;
  st.cacheAlertFired = true;
  useToastStore().push(
    "warn",
    i18n.global.t("usage.cacheAlert", { pct: dev.cacheHitAlertPercent }),
  );
}
