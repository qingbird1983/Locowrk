/**
 * 斜杠命令注册表（v2.143，UI shell §3.36① 扩展）：composer 输入 `/` 弹
 * 面板（选择/点击直接触发），键入继续过滤。命令的唯一真源——面板展示与
 * chat store 的提交拦截共用这里的 name/aliases，新增命令两处各加一行。
 * 描述是 UI 文案，descKey 落 locales 的 slash.* 区（渲染期翻译）。
 */
export interface SlashCommand {
  /** 命令主体（不含 `/`；提交与拦截都用 `/` + name 全等匹配）。 */
  name: string;
  /** 英文别名（提交口同样接受）。 */
  aliases?: string[];
  /** 描述 i18n key（slash.desc.*）。 */
  descKey: string;
}

export const SLASH_COMMANDS: SlashCommand[] = [
  {
    name: "整理",
    aliases: ["organize"],
    descKey: "slash.desc.organize",
  },
];

/** 按键入前缀过滤（匹配 name 或别名，大小写不敏感；空前缀 = 全列）。 */
export function filterSlashCommands(query: string): SlashCommand[] {
  const q = query.trim().toLowerCase();
  if (!q) return SLASH_COMMANDS;
  return SLASH_COMMANDS.filter(
    (c) =>
      c.name.toLowerCase().includes(q) ||
      (c.aliases ?? []).some((a) => a.toLowerCase().includes(q)),
  );
}
