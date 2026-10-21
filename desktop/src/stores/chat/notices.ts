// stores/chat/notices.ts —— 系统通知条的生产口 + M0 观测告警（T22 刀二搬出）。
// S4 刀② 记忆面退役后「整理发丝线」这个生产口删除，只剩通用条 / 压缩发丝线 / 原文条。
// 这一族只做一件事：**往 items 里落「UI 陈述」**（SystemNotice 模式），不发请求、不碰后端。
// 参数名一律 `st`（不是 `s`）：原实现内部的 `.find((s) => …)` 用的是 `s`，
// 把 store 状态参数也叫 `s` 会形成遮蔽 ⇒ 等价比对时看不清哪层是谁。
import { i18n } from "../../i18n";
import {
  COMPACT_TAG_AUTO,
  COMPACT_TAG_MANUAL,
  COMPACT_TAG_RESULT,
} from "../../lib/chat/compact-line";
import { useSettingsStore } from "../settings";
import { useToastStore } from "../toast";
import type { SessionUsage } from "./types";
import { nextId } from "./ids";
import type { ChatState } from "./state";
import type { SystemItem } from "./types";

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
  body?: string,
) {
  st.items.push({
    id: nextId(),
    kind: "system",
    key,
    params,
    actions,
    tone: "hairline",
    tag,
    body,
  });
}

function rewriteOrPushLine(
  st: ChatState,
  tag: string,
  runningKey: string,
  key: string,
  params?: Record<string, unknown>,
  actions?: string[],
  body?: string,
) {
  const items = st.items;
  for (let i = items.length - 1; i >= 0; i--) {
    const it = items[i]!;
    if (it.kind === "system" && it.tag === tag && it.key === runningKey) {
      it.key = key;
      it.params = params;
      it.actions = actions;
      it.body = body;
      return;
    }
  }
  pushLine(st, tag, key, params, actions, body);
}

/** 压缩发丝线（tag="compact"，S2 刀⑦ `/压缩`）：发起落「正在压缩」，收束就地改写
 * 成结果线（一次摘要往返要几秒，人得知道按钮生效了）。 */
export function pushCompactNotice(
  st: ChatState,
  key: string,
  params?: Record<string, unknown>,
  actions?: string[],
  body?: string,
) {
  rewriteOrPushLine(st, COMPACT_TAG_MANUAL, "notice.compactRunning", key, params, actions, body);
}

export function dropCompactRunning(st: ChatState) {
  dropRunningLine(st, COMPACT_TAG_MANUAL, "notice.compactRunning");
}

/** 后端判据名 → 语言包那条 key（specs/13 契约 C3）。取值域与 Rust 侧
 * `compact::REASONS` 同源，由 `check-single-source` 的 SS24 钉住：那边新增一条
 * 而这边没补文案，闸就红。
 *
 * ⚠ 兜底那条不许省：认不出的 reason 也要如实说"没压成"并把判据名原样带出来，
 * 绝不许悄悄显示成"已压缩"——那正是本项目一路在反的假告。
 *
 * **这一份住在 notices.ts 而不是 send.ts**（specs/12 刀②）：自动压缩那条路
 * （`agent://compact-status` 的 `failed` 相位）也要按判据名取文案，两条路共用
 * 同一张表 ⇒ 一份事实一个出处。搬过来之前它在 send.ts 里，而 send.ts 是"发东西
 * 的那一族"——判据名表放那儿等于让"读面"去读"写面"的私有物。
 *
 * ⚠ specs/13 起这张表**换了一整套**（跟着上游的失败分类走）：旧的
 * `unknown-window` / `too-few-rounds` / `summary-call` / `overflow` / `rejected`
 * 等判据名退役；熔断（`fused`）**整个不叫这个名字**——它走 `paused` 相位。 */
const COMPACT_SKIP_KEYS: Record<string, string> = {
  "nothing-to-compact": "notice.compactNothingToCompact",
  "template-mismatch": "notice.compactTemplateMismatch",
  "no-summary": "notice.compactNoSummary",
  "output-limit": "notice.compactOutputLimit",
  "cannot-shrink": "notice.compactCannotShrink",
  "provider-error": "notice.compactProviderError",
  "content-filter": "notice.compactContentFilter",
  interrupted: "notice.compactInterrupted",
  budget: "notice.compactBudget",
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
const COMPACT_AUTO_TAG = COMPACT_TAG_AUTO;

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
  actions?: string[],
  body?: string,
) {
  for (let i = st.items.length - 1; i >= 0; i--) {
    const it = st.items[i]!;
    if (it.kind === "system" && it.tag === COMPACT_AUTO_TAG) {
      it.key = key;
      it.params = params;
      it.actions = actions;
      it.body = body;
      return;
    }
  }
  pushLine(st, COMPACT_AUTO_TAG, key, params, actions, body);
}

/** 在途那根线的两种 key：「正在压缩」与「已压缩（等落账）」。
 *
 * 判"在途"按 **tag + key 两格**一起认：收束之后那根线的 tag 换成了
 * `compact-result` ⇒ 只看 tag 会把**已收束**的旧线也算进来（那是上一趟的
 * 正文），只看 key 又认不出手动那根与自动那根（它们 key 相同、tag 不同）。 */
const COMPACT_INFLIGHT_KEYS = new Set(["notice.compactRunning", "notice.compactDone"]);

/** 最后一根**在途**的压缩发丝线（= 正在流式写摘要 / 已压成待落账的那一根）。
 *
 * 用来把**流式摘要正文**挂到那根线的展开体上：摘要正文不在转录里另起一块
 * （那会变成第二种形态），而是挂在这根线自己身上——压缩这件事在界面上的形态
 * 仍然只有那根发丝线 + 一个展开入口（口径见《Front-end Design Guidelines》
 * §二⑲）。
 *
 * **两根线都算**（手动 tag=`compact` / 自动 tag=`compact-auto`）：后端那条
 * `delta` 相位不分手动自动（上游 `Compaction.Delta` 同款），只认自动那一根
 * 的话，手动 `/压缩` 流回来的摘要就没人接——事件发了、界面不动。
 */
export function lastCompactInflightLine(st: ChatState): SystemItem | undefined {
  for (let i = st.items.length - 1; i >= 0; i--) {
    const it = st.items[i]!;
    if (it.kind !== "system") continue;
    if (it.tag !== COMPACT_TAG_MANUAL && it.tag !== COMPACT_AUTO_TAG) continue;
    if (!COMPACT_INFLIGHT_KEYS.has(it.key)) continue;
    return it;
  }
  return undefined;
}

/** 这一趟压完了（`done` 相）：线**不收走**，就地改写成"已压缩"那一句。
 *
 * 为什么不直接收走：`context_compacted` 事件（带着"省了多少"与摘要全文）
 * 随后就到，在那之前这根线若被摘掉，界面上会闪一下"什么都没有"；而人此刻
 * 可能正展开着它看摘要流——收走 = 把人正在读的东西抽走。 */
export function markCompactDone(st: ChatState) {
  const line = lastCompactInflightLine(st);
  if (!line) return;
  line.key = "notice.compactDone";
  line.actions = ["details"];
}

/** 压成了 ⇒ 把在途那根线**就地改写**成结果线（省了多少 + 摘要全文 + 可撤销）。
 *
 * 为什么就地改写而不另起一条：同一趟压缩在界面上**只有一根线**（发起 → 收束
 * 是同一条线换说法），展开态也跟着留下来（人正看着的摘要不会凭空消失）。
 * 旧形态是"收走在途线 + 另起一张卡" ⇒ 点一次「查看详情」多一张、点几次
 * 堆几张（用户 2026-10-22 点名的 bug）。
 *
 * 返回 false = 没找到在途那根（重放 / 竞态）⇒ 调用方另起一条结果线。 */
export function settleCompactLine(
  st: ChatState,
  params: Record<string, unknown>,
  body: string,
): boolean {
  const line = lastCompactInflightLine(st);
  if (!line) return false;
  line.key = "notice.contextCompacted";
  line.params = params;
  line.actions = ["details", "undo"];
  line.body = body;
  line.tag = COMPACT_TAG_RESULT;
  return true;
}

/** 展开 / 收起压缩线的正文：**切换那根线自己的状态**。
 *
 * 不往转录里追加条目（追加 = 点一次多一张卡，见上）。 */
export function toggleCompactBody(item: SystemItem) {
  item.expanded = !item.expanded;
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
