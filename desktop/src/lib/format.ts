/**
 * 展示格式化助手（UI shell doc §3.5 数字呈现规范的唯一落点）： timestamp、
 * token 数、字节数等"给人看的数字"只在这里换算，页面不自带第二把尺。
 */

/** 相对时间的结构化结果：i18n key + 数参（前端 t() 渲染，§8.1 后端/工具
 * 只给结构化数据、文案归 locales 的同款纪律）。 */
export interface RelativeTime {
  /** time.* 下的键（S4 刀② 从 `memory.time.*` 迁出——相对时间是公共展示面，
   * 不该寄居在已退役的记忆文案域）。 */
  key: string;
  n?: number;
}

/**
 * **单位契约自此进函数名**（v2.168 批 F-18，C10 顺带抓到一条真缺陷）：旧
 * `relativeTime` 只接 Unix 秒，而调用点的数据源其实分两族——通知
 * （`notifications.rs` 的 `now_secs`）、会话 mtime（`persist.rs` 注释明写
 * Unix 秒）是秒；`…Ms` 这类数据源（目录快照 updatedMs 等）是毫秒。把毫秒当
 * 秒算 ⇒ `diffMs` 变**负** ⇒ 恒命中 `minutes < 1` ⇒ 永远显示「刚刚」。
 * 现在两个入口按单位命名，单位写错在调用点根本编译不出来。
 */

/** Unix 秒 → 相对时间档位（刚刚 / n 分钟前 / … / n 个月前）。 */
export function relativeTimeSec(tsSec: number, nowMs = Date.now()): RelativeTime {
  const diffMs = nowMs - tsSec * 1000;
  const minutes = Math.floor(diffMs / 60_000);
  if (minutes < 1) return { key: "just_now" };
  if (minutes < 60) return { key: "minutes", n: minutes };
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return { key: "hours", n: hours };
  const days = Math.floor(hours / 24);
  if (days < 7) return { key: "days", n: days };
  const weeks = Math.floor(days / 7);
  if (weeks < 5) return { key: "weeks", n: weeks };
  return { key: "months", n: Math.floor(days / 30) };
}

/** 毫秒时间戳 → 同一套档位（`…Ms` 这类数据源走这里）。 */
export function relativeTimeMs(tsMs: number, nowMs = Date.now()): RelativeTime {
  return relativeTimeSec(tsMs / 1000, nowMs);
}

/**
 * 档位 → 人类文案的**唯一渲染处**（§8.1「只给结构化数据、文案归 locales」）。
 * 此前五个调用点各写一遍「`n` 有没有就分两个 `t()`」的同形逻辑（其中
 * `LlmSection.vue` 少写分支、直接 `t(key, { n: undefined })`——等价，因为
 * `time.just_now` 文案里不含 `{n}` 占位，本批判据按两份语言的实文案核过）。
 * `tr` 注入而不是 import i18n：本文件保持可被纯逻辑判据直接驱动。
 */
export function relativeTimeLabel(
  rt: RelativeTime,
  tr: (key: string, params: Record<string, number>) => string,
): string {
  // `just_now` 档传空对象而不是省略第二参：vue-i18n 的 `t()` 重载里第二参是
  // **必填** `Record<string, unknown>`（传 undefined = TS2769，本批 vue-tsc 实测），
  // 而该档文案两份语言包都不含 `{n}` 占位 ⇒ 空对象对输出零影响（判据逐例核过）。
  return rt.n === undefined
    ? tr(`time.${rt.key}`, {})
    : tr(`time.${rt.key}`, { n: rt.n });
}

/**
 * 大数缩写（§3.5：262,144 → 256K、52,428.8 → ≈51.2K）：K/M 档、token
 * 语境按 1024 进位（262,144 = 256×1024，"256K" 才是用户心里的数）、
 * 整数不带 ≈、非整数一位小数带 ≈。S2 数字呈现批次接线（ContextSection
 * 预设/阈值预览、ContextProgressBar 比例段共用本函数），先备好唯一实现。
 */
export function compactNumber(n: number): string {
  if (n >= 1024 ** 2) return `${short(n / 1024 ** 2)}M`;
  if (n >= 1024) return `${short(n / 1024)}K`;
  return String(Math.round(n));
}

function short(v: number): string {
  const r = Math.round(v);
  if (Math.abs(v - r) < 1e-9) return String(r);
  return `≈${Math.round(v * 10) / 10}`;
}

/** 长文本摘引（通知摘要 / 工具卡折叠行参数摘要，§3.7）：截断加省略号，
 * 全文在展开体里。 */
export function clip(s: string, n: number): string {
  return s.length > n ? s.slice(0, n) + "…" : s;
}

/** 文件名摘引（§3.18 折叠行）：超长时**中段省略、保住扩展名**
 * （`permission-model-rewrite.ts` → `permission-mo…e.ts`）。与 `clip` 的差别只在
 * 尾部——切尾会把 `.ts` 吃掉，而文件类型正是折叠行里"这是什么文件"的唯一线索。
 * 无扩展名（或未超长）时退化为普通截断。 */
export function clipName(name: string, n: number): string {
  if (name.length <= n) return name;
  const dot = name.lastIndexOf(".");
  const ext = dot > 0 ? name.slice(dot) : "";
  const head = n - ext.length - 1;
  if (head < 1) return clip(name, n);
  return name.slice(0, head) + "…" + ext;
}

/** 时长人类呈现（§3.7 头行 / §3.10 子 Agent 卡共用一把尺）：`36s` /
 * `1m 6s` / `1h 2m`。 */
export function formatDuration(ms: number): string {
  const sec = Math.max(0, Math.round(ms / 1000));
  if (sec < 60) return `${sec}s`;
  if (sec < 3600) return `${Math.floor(sec / 60)}m ${sec % 60}s`;
  return `${Math.floor(sec / 3600)}h ${Math.floor((sec % 3600) / 60)}m`;
}
