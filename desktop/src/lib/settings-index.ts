/**
 * 设置索引（UI shell doc §3.32 一）：导航轨与 ⌘K 命令面板的唯一真源。
 * 每个分区声明 { id, group, labelKey, icon, searchKeys }，两侧同吃一份——
 * "新增分区忘了加搜索"这类漂移在结构上不可能发生。§3.2 的"settings.json
 * 分区 ↔ 面板分区一一对应"由 SettingsPage 按本索引渲染机械保证。
 *
 * searchKeys = 分区内行标题的 i18n key（搜索匹配的是解析后的文案，中英
 * 各自命中；索引驱动，不爬渲染后的 DOM）。
 */
import type { IconName } from "./icons";
import type { Composer } from "vue-i18n";

export type SettingsGroupId = "prefs" | "agent" | "capability" | "system";

export type SettingsSectionId =
  | "appearance"
  | "language"
  | "shortcut"
  | "chat"
  | "llm"
  | "context"
  | "memory"
  | "embedding"
  | "plugins"
  | "shell"
  | "tools"
  | "sessions"
  | "developer";

export interface SettingsGroup {
  id: SettingsGroupId;
  labelKey: string;
}

export interface SettingsSection {
  id: SettingsSectionId;
  group: SettingsGroupId;
  labelKey: string;
  icon: IconName;
  searchKeys: string[];
  /** 开发者分区（§3.2）：默认隐藏——解锁态（settings.developer.unlocked）
   * 才可见；导航轨 / ⌘K / 面板三处同过滤（§七 规则 16②）。 */
  devOnly?: boolean;
}

/** §3.32 二：分区 → 四组（偏好 / 智能体 / 能力与安全 / 系统）。组内顺序
 * 即渲染顺序（不排序）。「对话」「关于与更新」分区未实装，随各自批次补
 * 索引条目——不发明占位项。 */
export const SETTINGS_GROUPS: SettingsGroup[] = [
  { id: "prefs", labelKey: "settings.groups.prefs" },
  { id: "agent", labelKey: "settings.groups.agent" },
  { id: "capability", labelKey: "settings.groups.capability" },
  { id: "system", labelKey: "settings.groups.system" },
];

export const SETTINGS_SECTIONS: SettingsSection[] = [
  {
    id: "appearance",
    group: "prefs",
    labelKey: "settings.nav.appearance",
    icon: "sparkle",
    searchKeys: ["settings.themeTitle", "settings.themeSystem", "settings.themeLight", "settings.themeDark"],
  },
  {
    id: "language",
    group: "prefs",
    labelKey: "settings.nav.language",
    icon: "globe",
    searchKeys: ["settings.langTitle", "settings.langSystem", "settings.langZh", "settings.langEn"],
  },
  {
    id: "shortcut",
    group: "prefs",
    labelKey: "settings.nav.shortcut",
    icon: "edit",
    searchKeys: ["settings.shortcutTitle", "settings.shortcut"],
  },
  {
    id: "chat",
    group: "prefs",
    labelKey: "settings.nav.chat",
    icon: "chat",
    searchKeys: [
      "settings.chatWidthTitle",
      "settings.expandToolTitle",
      "settings.enterToSendTitle",
      "settings.usageReadoutTitle",
    ],
  },
  {
    id: "llm",
    group: "agent",
    labelKey: "settings.nav.llm",
    icon: "bot",
    searchKeys: ["settings.llmTitle", "settings.provider", "settings.model", "settings.baseUrl", "settings.apiKey"],
  },
  {
    id: "context",
    group: "agent",
    labelKey: "settings.nav.context",
    icon: "panel",
    searchKeys: ["settings.contextTitle"],
  },
  {
    id: "memory",
    group: "agent",
    labelKey: "settings.nav.memory",
    icon: "database",
    searchKeys: ["settings.memoryTitle"],
  },
  {
    id: "embedding",
    group: "agent",
    labelKey: "settings.nav.embedding",
    icon: "search",
    searchKeys: ["settings.embeddingTitle"],
  },
  {
    id: "plugins",
    group: "capability",
    labelKey: "settings.nav.plugins",
    icon: "plug",
    searchKeys: ["settings.pluginsTitle", "settings.fsRootsTitle"],
  },
  {
    id: "shell",
    group: "capability",
    labelKey: "settings.nav.shell",
    icon: "terminal",
    searchKeys: ["settings.shellTitle"],
  },
  {
    id: "tools",
    group: "capability",
    labelKey: "settings.nav.tools",
    icon: "checklist",
    searchKeys: ["settings.developerTitle", "settings.panelTitle", "settings.maxTurnSteps"],
  },
  {
    id: "sessions",
    group: "system",
    labelKey: "settings.nav.sessions",
    icon: "folder",
    searchKeys: ["settings.sessionsTitle", "settings.dataDir"],
  },
  {
    id: "developer",
    group: "system",
    labelKey: "settings.nav.developer",
    icon: "terminal",
    searchKeys: [
      "developer.allowDeleteRunning",
      "developer.allowSwitchRunning",
      "developer.hide",
    ],
    devOnly: true,
  },
];

/** 分区可见性：devOnly 分区仅在开发者选项解锁后出现（§3.2）。 */
export function isSectionVisible(
  section: SettingsSection,
  devUnlocked: boolean,
): boolean {
  return !section.devOnly || devUnlocked;
}

/** 分区按组归类（导航轨与面板共用同一份顺序）。 */
export function sectionsByGroup(groupId: SettingsGroupId): SettingsSection[] {
  return SETTINGS_SECTIONS.filter((s) => s.group === groupId);
}

/** 分区可搜索文案 = 轨内标签 + 分区内行标题（解析当前 locale 后拼接）。 */
export function sectionSearchText(section: SettingsSection, t: Composer["t"]): string {
  return [t(section.labelKey), ...section.searchKeys.map((k) => t(k))]
    .join(" ")
    .toLowerCase();
}
