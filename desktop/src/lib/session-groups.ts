/**
 * 会话侧栏分组（UI shell 文档 §2.3）：一级 = 工作区（v2.50 关联，
 * `session.workspace` = fsRoots 根目录路径），二级 = 时间桶（近 7 天 /
 * 近 14 天 / 更早，按最后活动时间前端即可算）。纯函数无副作用，
 * Sidebar 只渲染。
 */
import type { SessionMeta } from "../types";

export type BucketKey = "bucket7" | "bucket14" | "earlier";

export interface BucketGroup {
  key: BucketKey;
  items: SessionMeta[];
}

export interface WorkspaceGroup {
  /** 折叠键 = workspace 存储路径（§2.3 头行可折叠，键随路径稳定）。 */
  key: string;
  /** 显示名 = 存储路径末段——根目录后来从设置移除的组照常显示。 */
  name: string;
  buckets: BucketGroup[];
  /** 组内含运行中会话（§2.3 头行尾随 6px 圆点）。 */
  running: boolean;
}

export interface SessionTree {
  /** 顶层「会话」组（未关联会话）的时间桶（空桶已滤）。 */
  top: BucketGroup[];
  /** 「项目」组，按组内最近活动降序（空桶已滤）。 */
  workspaces: WorkspaceGroup[];
}

const DAY = 86_400_000;
const BUCKETS: { key: BucketKey; limit: number }[] = [
  { key: "bucket7", limit: 7 * DAY },
  { key: "bucket14", limit: 14 * DAY },
  { key: "earlier", limit: Number.POSITIVE_INFINITY },
];

function bucketIndex(updatedAtSeconds: number): number {
  const age = Date.now() - updatedAtSeconds * 1000;
  const idx = BUCKETS.findIndex((b) => age <= b.limit);
  return idx === -1 ? BUCKETS.length - 1 : idx;
}

/** 工作区显示名：路径末段（Windows `\` 与 POSIX `/` 通吃）。 */
export function workspaceDisplayName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

function emptyBuckets(): BucketGroup[] {
  return BUCKETS.map((b) => ({ key: b.key, items: [] }));
}

/** 把会话列表（一般 mtime 降序）组装成 §2.3 两级树。`fsRoots` = 设置
 * 里的工作区根目录——§2.3 定义「工作区 = fsRoots 里的一个根目录」，
 * 已配置但尚无会话归属的根照常成组（空组只渲染头行）；组集合 =
 * fsRoots ∪ 已关联会话的 workspace（根目录后来移除的组照常显示）。 */
export function groupSessions(
  sessions: SessionMeta[],
  runningId: string | null,
  fsRoots: string[] = [],
): SessionTree {
  const top = emptyBuckets();
  const wsMap = new Map<string, { group: WorkspaceGroup; latest: number }>();
  for (const s of sessions) {
    const idx = bucketIndex(s.updatedAt);
    if (!s.workspace) {
      top[idx].items.push(s);
      continue;
    }
    let entry = wsMap.get(s.workspace);
    if (!entry) {
      entry = {
        group: {
          key: s.workspace,
          name: workspaceDisplayName(s.workspace),
          buckets: emptyBuckets(),
          running: false,
        },
        latest: 0,
      };
      wsMap.set(s.workspace, entry);
    }
    entry.group.buckets[idx].items.push(s);
    entry.latest = Math.max(entry.latest, s.updatedAt);
    if (s.id === runningId) entry.group.running = true;
  }
  // 空工作区组（fsRoots 配置了但还没有会话归属）垫后，保持 fsRoots 顺序
  for (const root of fsRoots) {
    if (!wsMap.has(root)) {
      wsMap.set(root, {
        group: {
          key: root,
          name: workspaceDisplayName(root),
          buckets: [],
          running: false,
        },
        latest: Number.NEGATIVE_INFINITY,
      });
    }
  }
  // 有会话的组按组内最近活动降序；空组（-Infinity）稳定排在末尾
  const workspaces = [...wsMap.values()]
    .sort((a, b) => {
      if (a.latest === b.latest) return 0;
      if (a.latest === Number.NEGATIVE_INFINITY) return 1;
      if (b.latest === Number.NEGATIVE_INFINITY) return -1;
      return b.latest - a.latest;
    })
    .map((e) => {
      e.group.buckets = e.group.buckets.filter((b) => b.items.length > 0);
      return e.group;
    });
  return { top: top.filter((b) => b.items.length > 0), workspaces };
}
