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
    // S2 刀⑦（AC-S2-11）：旧名 `/整理` 退役。它现在做的是"当场跑一次主层压缩"，
    // 与记忆树那套章节整理无关 ⇒ 名字跟着行为改，两家参照的同位按钮也都叫 compact。
    // 旧名**不留别名**：让 `/整理` 继续被拦却干压缩的事，就是 ⓓ/裁定 B 一路在清的
    // 那种名字与事实不符。
    name: "压缩",
    aliases: ["compact"],
    descKey: "slash.desc.compact",
  },
];

/** 这条文本是不是命令 `name`（含别名）。提交口用它而不是再敲一遍 `"/压缩"` 字面量：
 * 原先名字在注册表与 `send.ts` 各存一份，改一次名要记得动两处（刀⑦ 正是那次改名，
 * 所以顺手收成一处——漏一处的后果是"面板里有这条、敲下去却当普通消息发出去"）。 */
export function isSlashCommand(text: string, name: string): boolean {
  const cmd = SLASH_COMMANDS.find((c) => c.name === name);
  if (!cmd) return false;
  const typed = text.trim().replace(/^\/+/, "");
  return typed === cmd.name || (cmd.aliases ?? []).includes(typed);
}

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
