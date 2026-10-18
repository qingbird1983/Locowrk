/** **设置读写**（DESIGN.md §13.1 / §13.5）：读投影（永不含密钥原值）与部分更新。
 * 更新参数那十二个区就是类型面里的十二个 Update 形状，本片是它们**进前端**的唯一入口。 */

import { invoke } from "@tauri-apps/api/core";
import type {
  AppearanceUpdate,
  ChatUpdate,
  ContextUpdate,
  DeveloperUpdate,
  EmbeddingUpdate,
  LlmUpdate,
  MemoryUpdate,
  PluginsUpdate,
  SettingsView,
  ToolsUpdate,
} from "../../types";
// ---- 设置（DESIGN.md §13.1 / §13.5） ----

/** 读设置投影（永不含 API Key 原值）。 */
export function getSettings(): Promise<SettingsView> {
  return invoke("settings_get");
}

/** 部分更新设置，返回最新投影。 */
export function setSettings(patch: {
  locale?: string;
  appearance?: AppearanceUpdate;
  shortcut?: string;
  llm?: LlmUpdate;
  embedding?: EmbeddingUpdate;
  context?: ContextUpdate;
  memory?: MemoryUpdate;
  tools?: ToolsUpdate;
  plugins?: PluginsUpdate;
  chat?: ChatUpdate;
  developer?: DeveloperUpdate;
  thinking?: { language: string };
}): Promise<SettingsView> {
  return invoke("settings_set", {
    locale: patch.locale ?? null,
    appearance: patch.appearance ?? null,
    shortcut: patch.shortcut ?? null,
    llm: patch.llm ?? null,
    embedding: patch.embedding ?? null,
    context: patch.context ?? null,
    memory: patch.memory ?? null,
    tools: patch.tools ?? null,
    plugins: patch.plugins ?? null,
    chat: patch.chat ?? null,
    developer: patch.developer ?? null,
    thinking: patch.thinking ?? null,
  });
}
