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
  /** §2.3 分组头折叠键（v2.50：工作区组 = workspace 存储路径）。 */
  collapsedGroups: string[];
}

export function loadSidebarPrefs(): SidebarPrefs {
  const fallback: SidebarPrefs = { width: SIDEBAR_DEFAULT, collapsed: false, collapsedGroups: [] };
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as Partial<SidebarPrefs>;
    const width =
      typeof parsed.width === "number" && Number.isFinite(parsed.width)
        ? Math.min(SIDEBAR_MAX, Math.max(SIDEBAR_MIN, Math.round(parsed.width)))
        : SIDEBAR_DEFAULT;
    return {
      width,
      collapsed: parsed.collapsed === true,
      collapsedGroups: Array.isArray(parsed.collapsedGroups)
        ? parsed.collapsedGroups.filter((k): k is string => typeof k === "string")
        : [],
    };
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

/** §2.3 分组头折叠态（v2.50）：读 / 读改写（与宽/折叠同文件互不覆盖）。 */
export function loadCollapsedGroups(): Set<string> {
  return new Set(loadSidebarPrefs().collapsedGroups);
}

export function saveCollapsedGroups(groups: Set<string>): void {
  const prefs = loadSidebarPrefs();
  prefs.collapsedGroups = [...groups];
  saveSidebarPrefs(prefs);
}
