/**
 * 插件版本与展示的纯助手（无 Vue 依赖）：市场 latest 判定、可更新/
 * 可回退集合、多语言展示名。发现页与已安装页共用。
 */
import { pickLocalized } from "./i18n-helpers";
import type { InstalledPlugin, MarketplaceItem } from "../types";

/** 只前向比较的版本序：a > b ⇒ 1（§13.3 可更新判定，不做语义化解析）。 */
export function cmpVer(a: string, b: string): number {
  const pa = a.split(".").map(Number);
  const pb = b.split(".").map(Number);
  for (let i = 0; i < 3; i++) {
    const d = (pa[i] || 0) - (pb[i] || 0);
    if (d) return d;
  }
  return 0;
}

/** 市场条目的 latest（versions 升序，末位 latest）。 */
export function latestOf(
  market: MarketplaceItem[],
  id: string,
): string | undefined {
  const versions = market.find((m) => m.id === id)?.versions;
  return versions ? versions[versions.length - 1] : undefined;
}

/** 本地版本 < 市场 latest ⇒ 可更新。 */
export function updateAvailable(
  market: MarketplaceItem[],
  p: InstalledPlugin,
): boolean {
  const latest = latestOf(market, p.id);
  return latest !== undefined && cmpVer(latest, p.version) > 0;
}

/** 可回退的版本：本进程加载过、且不是当前生效版本的。 */
export function rollbackTargets(p: InstalledPlugin): string[] {
  return p.loaded_versions.filter((v) => v !== p.active_version && v !== p.version);
}

/** 展示名：display_name 多语言回退链，缺省用 id。 */
export function displayName(
  p: { id: string; display: { display_name?: Record<string, string> } },
  locale: string,
): string {
  return pickLocalized(p.display.display_name, locale, p.id);
}
