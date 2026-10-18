/** 工具明细的**视图模型类型**（台账 T36 五片之首）：翻译函数签名 `Tr`、渲染上下文
 * `ToolDetailCtx`、属性行 `PropRow`、列表行 `ListRow`、块联合 `DetailBlock`、出口 `ToolDetail`。
 * 只有类型没有实现——五个面共用的那一份形状定义，别处再写一遍就是第二个出处。
 *
 * 这些名字由 `lib/tool-detail.ts` 原样再导出，消费点（`ToolRow.vue` / `ToolDetailView.vue`）
 * 的 import 路径一个字不改。 */
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
  | { kind: "entry"; childId: string; title: string };

export interface ToolDetail {
  blocks: DetailBlock[];
}
