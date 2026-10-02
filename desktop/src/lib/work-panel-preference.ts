/**
 * 工作面板偏好持久化（UI shell 文档 §2.5，v2.52）：开合与宽度跨重启保留
 * （页签是进程内壳状态，不跨重启——§3.16 转录文件在，可从子 Agent 卡重
 * 开）。localStorage 同侧栏偏好族；解析失败静默回缺省。
 */

const KEY = "locowrk.work-panel.v1";

export const PANEL_MIN = 244;
export const PANEL_DEFAULT = 360;
/** 主区地板（§2.5 硬约束）：composer 工具条不折行的宽度。 */
export const MAIN_FLOOR = 450;

export interface WorkPanelPrefs {
  open: boolean;
  width: number;
}

export function loadPanelPrefs(): WorkPanelPrefs {
  const fallback: WorkPanelPrefs = { open: false, width: PANEL_DEFAULT };
  try {
    const raw = localStorage.getItem(KEY);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as Partial<WorkPanelPrefs>;
    const width =
      typeof parsed.width === "number" && Number.isFinite(parsed.width)
        ? Math.max(PANEL_MIN, Math.round(parsed.width))
        : PANEL_DEFAULT;
    return { open: parsed.open === true, width };
  } catch {
    return fallback;
  }
}

export function savePanelPrefs(prefs: WorkPanelPrefs): void {
  try {
    localStorage.setItem(KEY, JSON.stringify(prefs));
  } catch {
    /* 存不下只丢偏好 */
  }
}
