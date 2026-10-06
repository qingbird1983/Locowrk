/**
 * 会话侧栏分组（UI shell 文档 §2.3，v3.9 项目接管）：一级 = 项目（工作区，
 * v2.50 关联，`session.workspace` = fsRoots 根目录路径），项目下任务按最近
 * 活动直排（v3.9 时间桶退役）。**侧栏只保留项目工作区**（用户裁定）——
 * 无归属会话不进树（⌘K 面板照列全部，行菜单可移入项目归位）。
 * 纯函数无副作用，SessionGroups 只渲染。
 */
import type { SessionMeta } from "../types";

export interface WorkspaceGroup {
  /** 折叠键 = workspace 存储路径（§2.3 头行可折叠，键随路径稳定）。 */
  key: string;
  /** 显示名 = 存储路径末段。 */
  name: string;
  /** 组内任务（最近活动降序）。 */
  items: SessionMeta[];
  /** 组内含运行中会话（§2.3 头行尾随 6px 圆点；「只看进行中」筛选项）。 */
  running: boolean;
}

export interface SessionTree {
  /** 「项目」组：有会话的按组内最近活动降序，fsRoots 空组垫后保配置序。
   * 无归属、或归属不在 fsRoots（项目已移除，v2.113）的会话不进侧栏树
   * （用户裁定：项目工作区全权接管；⌘K 面板照列全部会话，行菜单可移入
   * 项目归位）。 */
  workspaces: WorkspaceGroup[];
}

/** 工作区显示名：路径末段（Windows `\` 与 POSIX `/` 通吃）。 */
export function workspaceDisplayName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

export interface GroupOptions {
  /** false = 不补 fsRoots 空组（「只看进行中」筛模下空组没有意义）。 */
  includeEmpty?: boolean;
}

/** 把会话列表（一般 mtime 降序）组装成 §2.3 项目树。`fsRoots` = 设置里的
 * 工作区根目录——§2.3 定义「工作区 = fsRoots 里的一个根目录」，已配置但
 * 尚无会话归属的根照常成组（空组只渲染头行）；**组集合 = fsRoots**——
 * 归属不在 fsRoots 的会话不进树（v2.113 用户裁定「移除项目无效」修复：
 * 旧规则「根目录移除的组照常显示」使移除动作看起来无效；归属数据本身
 * 不动，重新添加同目录即整组恢复，⌘K 照列全部会话）。 */
export function groupSessions(
  sessions: SessionMeta[],
  runningId: string | null,
  fsRoots: string[] = [],
  options: GroupOptions = {},
): SessionTree {
  const includeEmpty = options.includeEmpty ?? true;
  const byRecency = [...sessions].sort((a, b) => b.updatedAt - a.updatedAt);
  const wsMap = new Map<string, WorkspaceGroup>();
  for (const s of byRecency) {
    if (!s.workspace) continue; // 项目工作区全权接管：无归属不进树
    // 项目已移除（不在 fsRoots）：组不进树（v2.113，见函数头注——归属
    // 数据不动，重新添加同目录整组恢复）
    if (!fsRoots.includes(s.workspace)) continue;
    let g = wsMap.get(s.workspace);
    if (!g) {
      g = {
        key: s.workspace,
        name: workspaceDisplayName(s.workspace),
        items: [],
        running: false,
      };
      wsMap.set(s.workspace, g);
    }
    g.items.push(s);
    if (s.id === runningId) g.running = true;
  }
  // 空工作区组（fsRoots 配置了但还没有会话归属）垫后，保持 fsRoots 顺序
  if (includeEmpty) {
    for (const root of fsRoots) {
      if (!wsMap.has(root)) {
        wsMap.set(root, {
          key: root,
          name: workspaceDisplayName(root),
          items: [],
          running: false,
        });
      }
    }
  }
  return { workspaces: [...wsMap.values()] };
}
