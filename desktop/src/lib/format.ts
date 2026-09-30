/**
 * 展示格式化助手（UI shell doc §3.5 数字呈现规范的唯一落点）： timestamp、
 * token 数、字节数等"给人看的数字"只在这里换算，页面不自带第二把尺。
 */

/** 相对时间的结构化结果：i18n key + 数参（前端 t() 渲染，§8.1 后端/工具
 * 只给结构化数据、文案归 locales 的同款纪律）。 */
export interface RelativeTime {
  /** memory.time.* 下的键。 */
  key: string;
  n?: number;
}

/** Unix 秒 → 相对时间档位（刚刚 / n 分钟前 / … / n 个月前）。 */
export function relativeTime(tsSeconds: number, nowMs = Date.now()): RelativeTime {
  const diffMs = nowMs - tsSeconds * 1000;
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
