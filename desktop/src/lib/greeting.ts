/**
 * 欢迎页问候（UI shell 文档 §3.14，v3.9 issue #13；v2.60 修复批）：时段
 * 四态 + 当前索引项目名。纯函数，组件只在挂载/时间跨档时取值（不计时器
 * ——问候粒度是"次打开看一眼"，跨档漂移几分钟内无感知）。
 */
import type { SessionMeta } from "../types";
import { workspaceDisplayName } from "./session-groups";

export type GreetingKey = "morning" | "afternoon" | "evening" | "lateNight";

/** 上午好 05:00–12:00 / 下午好 12:00–18:00 / 晚上好 18:00–23:00 / 夜深了 其余。 */
export function greetingKey(d: Date = new Date()): GreetingKey {
  const h = d.getHours();
  if (h >= 5 && h < 12) return "morning";
  if (h >= 12 && h < 18) return "afternoon";
  if (h >= 18 && h < 23) return "evening";
  return "lateNight";
}

/** 当前索引项目名（§3.14；v2.60：挂起归属最优先——项目上新建任务后问候
 * 立即跟着切）：挂起归属 → 当前会话的归属项目 → 最近活动会话的归属项目
 * → 第一个 fsRoot → null（无任何项目 = 不带名字的问候，v2.60 修复批）。
 * 旧版兜底 "Locowrk" 按用户裁定退役——兜底名不是"已选项目"。 */
export function homeProjectName(
  sessions: SessionMeta[],
  sessionId: string | null,
  fsRoots: string[] = [],
  pendingWorkspace: string | null = null,
): string | null {
  if (pendingWorkspace) return workspaceDisplayName(pendingWorkspace);
  const current = sessions.find((s) => s.id === sessionId)?.workspace;
  if (current) return workspaceDisplayName(current);
  const recent = sessions.find((s) => s.workspace);
  if (recent?.workspace) return workspaceDisplayName(recent.workspace);
  if (fsRoots.length > 0) return workspaceDisplayName(fsRoots[0]);
  return null;
}
