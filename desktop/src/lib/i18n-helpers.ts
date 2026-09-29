/**
 * 多语言展示工具（packaging.md §6.2）。回退链与 Rust 侧
 * plugin_api::manifest::LocalizedString::pick 一致：
 * 精确 → 主语言（zh-CN → zh*）→ en-US → 任意 → fallback。
 */
export function pickLocalized(
  obj: Record<string, string> | null | undefined,
  locale: string,
  fallback: string,
): string {
  if (!obj) return fallback;
  const exact = obj[locale];
  if (exact) return exact;

  const primary = locale.split("-")[0];
  const hit = Object.entries(obj).find(([k]) => k.split("-")[0] === primary);
  if (hit) return hit[1];

  return obj["en-US"] ?? Object.values(obj)[0] ?? fallback;
}
