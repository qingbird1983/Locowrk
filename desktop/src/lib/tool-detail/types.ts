/** 工具明细的**视图模型类型**（台账 T36 五片之首）：翻译函数签名 `Tr`、渲染上下文
 * `ToolDetailCtx`、属性行 `PropRow`、列表行 `ListRow`、块联合 `DetailBlock`、出口 `ToolDetail`。
 * 只有类型没有实现——五个面共用的那一份形状定义，别处再写一遍就是第二个出处。
 *
 * 这些名字由 `lib/tool-detail.ts` 原样再导出，消费点（`ToolRow.vue` / `ToolDetailView.vue`）
 * 的 import 路径一个字不改。 */
import type { DiffRow } from "../chat/diff";

/** 渲染期翻译函数（调用方从 useI18n 传入，lib 层不持有 i18n 实例）。 */
export type Tr = (key: string, params?: Record<string, unknown>) => string;

export interface ToolDetailCtx {
  /** 数据根 + fsRoots（工作区相对化用，与折叠行摘要同一来源）。 */
  roots: string[];
  tr: Tr;
}

export interface PropRow {
  label: string;
  value: string;
  /** mono = 等宽（命令/路径/代码值）；ok = 绿；danger = 红；缺省 = 正文灰。 */
  tone?: "mono" | "ok" | "danger";
}

export interface ListRow {
  icon: "file" | "folder" | "link" | "tool";
  /** 主文本（文件名 / 文件:行号 / 工具名）。 */
  title: string;
  /** 次文本（大小 / 行内容摘引 / 键值），等宽小字。 */
  meta?: string;
}

export type DetailBlock =
  | { kind: "props"; rows: PropRow[] }
  /** 代码内容 → `.md` 代码块组件复用（语言标签头行 + 复制钮）。 */
  | { kind: "code"; language: string; content: string }
  | { kind: "text"; content: string; danger?: boolean }
  | { kind: "list"; items: ListRow[] }
  /** 子代理转录入口（查看完整对话文本钮）。 */
  | { kind: "entry"; childId: string; title: string }
  /** 差异表（设计稿 §7.1）：写操作改了什么。四列 = 旧行号/新行号/符号/内容，
   * 加删行底色走 `--lw-diff-add-wash` / `-del-wash`（不借 ok/danger 色相）。 */
  | { kind: "diff"; rows: DiffRow[] }
  /** 终端块（设计稿 §7.3）：`$ 命令` 行 + stdout/stderr 合并输出，深底；
   * 运行中吸底跟随由渲染侧管，这里只给数据。 */
  | { kind: "term"; command: string; output: string; running?: boolean };

export interface ToolDetail {
  blocks: DetailBlock[];
}
