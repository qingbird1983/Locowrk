/**
 * 侧栏偏好持久化（UI shell doc §2.3 / §8.4 前端独力可做项）：宽度与折叠态
 * 跨重启保留。localStorage 足够——侧栏宽是壳偏好而非设置级数据（列宽才
 * 是，§2.7 走 settings.json）；解析失败静默回缺省。
 */

const KEY = "locowrk.sidebar-prefs.v1";

export const SIDEBAR_MIN = 240;
export const SIDEBAR_DEFAULT = 275;
export const SIDEBAR_MAX = 520;

export interface SidebarPrefs {
  /** 展开宽度（折叠 ≠ 宽度 0：折叠是独立状态，展开回到记住的宽度）。 */
  width: number;
  collapsed: boolean;
}

export function loadSidebarPrefs(): SidebarPrefs {
  const fallback: SidebarPrefs = { width: SIDEBAR_DEFAULT, collapsed: false };
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as Partial<SidebarPrefs>;
    const width =
      typeof parsed.width === "number" && Number.isFinite(parsed.width)
        ? Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, Math.round(parsed.width)))
        : SIDEBAR_DEFAULT;
    return { width, collapsed: parsed.collapsed === true };
  } catch {
    return fallback;
  }
}

export function saveSidebarPrefs(prefs: SidebarPrefs): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs));
  } catch {
    /* 存不下只丢偏好，不影响功能 */
  }
}
