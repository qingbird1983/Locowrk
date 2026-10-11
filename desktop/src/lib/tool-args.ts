/**
 * 工具参数摘要的两个公共核（F6 C8/C9 收敛，v2.168）。
 *
 * ① **命令剥包装**：`tool-summary.ts::commandSummary`（折叠行）与
 *    `tool-detail.ts::stripCommandWrapping`（展开明细）的正则段此前逐字相同，
 *    唯一真实差异 = 截断长度 48 / 40 ⇒ 收成一份，长度进参数。
 * ② **工作区相对化**：两处的**兜底形态本就不同**（折叠行不显示绝对盘符、只给裸
 *    文件名；明细要多留目录信息并剥 Windows 扩展路径前缀 `\\?\`，那是插件 display
 *    带来的纯显示噪声），而"落在某个根内就切掉根前缀"这段匹配逐字相同 ⇒
 *    **合并的是匹配核，不是把两种显示语义强行拧成一种**。两个出口按用途命名，
 *    调用点写错形态会在 code review 里露出来（同名同形 = 审计 C9 的病根）。
 */
import { clip } from "./format";

/**
 * 剥 `cd <dir> && ` 前缀与 `bash -lc "…"` 包装；`&&` 串联只留第一个有意义的命令，
 * 其余记 `… +N`（UI shell doc §3.18）。循环剥到底：`cd a && bash -lc "cd b && x"`
 * 这类叠套形态两份原实现都能剥穿，行为保持一致。
 */
export function stripCommandWrapping(raw: string, clipTo: number): string {
  let s = raw.trim();
  let hidden = 0;
  for (;;) {
    const cd = s.match(/^cd\s+(?:"[^"]*"|\S+)\s*&&\s*/);
    if (cd) {
      s = s.slice(cd[0].length);
      hidden++;
      continue;
    }
    const sh = s.match(/^(?:bash|sh)\s+(?:-l\w*\s+)*-c\s+"([\s\S]*)"\s*$/);
    if (sh) {
      s = sh[1]!.trim();
      continue;
    }
    break;
  }
  const parts = s.split(" && ");
  const first = parts[0]!.trim();
  hidden += Math.max(0, parts.length - 1);
  return clip(first, clipTo) + (hidden > 0 ? ` … +${hidden}` : "");
}

/** 路径落在某个根内 → 返回去掉根前缀的相对段；否则 null。两处的这段逐字相同。 */
function relWithinRoot(norm: string, roots: string[]): string | null {
  for (const root of roots) {
    const r = root.replace(/\\/g, "/").replace(/\/+$/, "");
    if (r && norm.toLowerCase().startsWith(r.toLowerCase() + "/")) {
      return norm.slice(r.length + 1);
    }
  }
  return null;
}

/**
 * 折叠行用（§3.18「能相对就相对，不显示绝对盘符」）：全路径留给展开明细与
 * tooltip，所以落不进根时只给裸文件名。不剥 `\\?\`——折叠行拿到的参数面没有它。
 */
export function relativizeBare(path: string, roots: string[]): string {
  const norm = path.replace(/\\/g, "/");
  const rel = relWithinRoot(norm, roots);
  if (rel !== null) return rel;
  const base = norm.split("/").pop() ?? norm;
  return base || norm;
}

/**
 * 展开明细属性行用：比折叠行的裸文件名多留目录信息 ⇒ 落不进根时保留整条归一化
 * 路径；顺带剥 Windows 扩展路径前缀 `\\?\`（插件 display 带它，纯显示噪声）。
 */
export function relativizeDetail(path: string, roots: string[]): string {
  const norm = path.replace(/\\\\\?\\/g, "").replace(/\\/g, "/");
  return relWithinRoot(norm, roots) ?? norm;
}
