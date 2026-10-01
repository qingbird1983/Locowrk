// 设置面板状态（§13.1 / §13.5）：投影 + 部分更新动作。
// 语言切换立即生效（applyLocalePref）并持久化到数据根 settings.json。
import { defineStore } from "pinia";
import { getSettings, setSettings } from "../lib/api";
import { applyLocalePref } from "../i18n";
import { applyThemePref } from "../lib/theme";
import type {
  ContextUpdate,
  EmbeddingUpdate,
  LlmUpdate,
  MemoryUpdate,
  PluginsUpdate,
  SettingsView,
  ShellUpdate,
  ToolsUpdate,
} from "../types";

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    view: null as SettingsView | null,
    /** LLM 表单保存中（按钮禁用、防连点）。 */
    saving: false,
    /** 最近一次保存刚成功（短暂显示“已保存”）。 */
    savedFlash: false,
    /** 最近一次失败提示（组件红条显示，下一动作清除）。 */
    error: "",
  }),
  actions: {
    /** App 启动时调用：拉投影 + 应用已存语言偏好。失败不阻塞启动。 */
    async init() {
      try {
        this.view = await getSettings();
        applyLocalePref(this.view.locale);
        applyThemePref(this.view.appearance.theme);
      } catch (e) {
        this.error = String(e);
      }
    },
    /** 切语言：立即生效，成功后以返回投影为准（失败时 select 回滚）。 */
    async setLocale(locale: string) {
      this.error = "";
      try {
        this.view = await setSettings({ locale });
        applyLocalePref(this.view.locale);
      } catch (e) {
        this.error = String(e);
        if (this.view) applyLocalePref(this.view.locale);
      }
    },
    /** 切主题（⑰）：立即生效（lib/theme.ts 写 <html data-theme>），
     * 成功后以返回投影为准（失败时 select 回滚）。 */
    async setTheme(theme: string) {
      this.error = "";
      try {
        this.view = await setSettings({ appearance: { theme } });
        applyThemePref(this.view.appearance.theme);
      } catch (e) {
        this.error = String(e);
        if (this.view) applyThemePref(this.view.appearance.theme);
      }
    },
    /** 保存 LLM 配置；空串字段语义 = 清除（后端 settings.rs）。 */
    async saveLlm(llm: LlmUpdate) {
      if (this.saving) return;
      this.error = "";
      this.saving = true;
      this.savedFlash = false;
      try {
        this.view = await setSettings({ llm });
        this.savedFlash = true;
        setTimeout(() => (this.savedFlash = false), 2000);
      } catch (e) {
        this.error = String(e);
      } finally {
        this.saving = false;
      }
    },
    /** 保存全局快捷键（tray 即时重注册；键位被占用时后端报错）。 */
    async saveShortcut(shortcut: string) {
      if (this.saving) return;
      this.error = "";
      this.saving = true;
      this.savedFlash = false;
      try {
        this.view = await setSettings({ shortcut });
        this.savedFlash = true;
        setTimeout(() => (this.savedFlash = false), 2000);
      } catch (e) {
        this.error = String(e);
      } finally {
        this.saving = false;
      }
    },
    /** 保存上下文管理区（⑨）：后端校验 + 换入 ctx-compact 活配置，
     * 保存即生效。校验失败（阈值出界等）在 error 条显示。 */
    async saveContext(context: ContextUpdate) {
      if (this.saving) return;
      this.error = "";
      this.saving = true;
      this.savedFlash = false;
      try {
        this.view = await setSettings({ context });
        this.savedFlash = true;
        setTimeout(() => (this.savedFlash = false), 2000);
      } catch (e) {
        this.error = String(e);
      } finally {
        this.saving = false;
      }
    },
    /** 保存记忆注入区（⑪）：后端校验 + 换入 Memory-Inject 活配置，
     * 保存即生效（下一轮请求就按新条数注入）。 */
    async saveMemory(memory: MemoryUpdate) {
      if (this.saving) return;
      this.error = "";
      this.saving = true;
      this.savedFlash = false;
      try {
        this.view = await setSettings({ memory });
        this.savedFlash = true;
        setTimeout(() => (this.savedFlash = false), 2000);
      } catch (e) {
        this.error = String(e);
      } finally {
        this.saving = false;
      }
    },
    /** 保存工具暴露区（⑫）：后端校验 + 换入 ToolExposure 活配置，
     * 保存即生效（下一轮请求按新上限截断）。 */
    async saveTools(tools: ToolsUpdate) {
      if (this.saving) return;
      this.error = "";
      this.saving = true;
      this.savedFlash = false;
      try {
        this.view = await setSettings({ tools });
        this.savedFlash = true;
        setTimeout(() => (this.savedFlash = false), 2000);
      } catch (e) {
        this.error = String(e);
      } finally {
        this.saving = false;
      }
    },
    /** 保存 embedding 区（⑬）：后端热重建共享客户端，检索/经历/提取
     * 随即生效。空串字段语义 = 清除。 */
    async saveEmbedding(embedding: EmbeddingUpdate) {
      if (this.saving) return;
      this.error = "";
      this.saving = true;
      this.savedFlash = false;
      try {
        this.view = await setSettings({ embedding });
        this.savedFlash = true;
        setTimeout(() => (this.savedFlash = false), 2000);
      } catch (e) {
        this.error = String(e);
      } finally {
        this.saving = false;
      }
    },
    /** 保存插件管理区（⑬）：自动停用天数（0 = 关）。天数 > 0 时后端
     * 保存即巡检一遍；关闭不复活已停用插件。 */
    async savePlugins(plugins: PluginsUpdate) {
      if (this.saving) return;
      this.error = "";
      this.saving = true;
      this.savedFlash = false;
      try {
        this.view = await setSettings({ plugins });
        this.savedFlash = true;
        setTimeout(() => (this.savedFlash = false), 2000);
      } catch (e) {
        this.error = String(e);
      } finally {
        this.saving = false;
      }
    },
    /** 保存 shell 治理区（S1）：高危解锁开关经插件加载前的 env 注入，
     * 重启后生效（分区文案里说明）。 */
    async saveShell(shell: ShellUpdate) {
      if (this.saving) return;
      this.error = "";
      this.saving = true;
      this.savedFlash = false;
      try {
        this.view = await setSettings({ shell });
        this.savedFlash = true;
        setTimeout(() => (this.savedFlash = false), 2000);
      } catch (e) {
        this.error = String(e);
      } finally {
        this.saving = false;
      }
    },
  },
});
