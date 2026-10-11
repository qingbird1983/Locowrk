// 设置面板状态（§13.1 / §13.5）：投影 + 部分更新动作。
// 语言切换立即生效（applyLocalePref）并持久化到数据根 settings.json。
//
// 保存类动作的形状（v2.169 批 F-19，审计 C13）：10 个 `saveXxx` 里此前各写一遍
// 「防连点门 → 清 error → `setSettings` → 投影回写 → 短暂『已保存』→ 失败进 error」
// 的同构体（262 行里约 110 行是这十份副本）。现在唯一实现 = `patchSettings`，
// `saveXxx` 退化成"带自己那个 patch 键的委托"。**两个真实例外按参数保留**，
// 理由写在各自注释里（收口不等于把它们强行拧成一种）：
//   · `saveThinking` 不闪「已保存」——落盘即成功，生效提示由 LlmSection 负责；
//   · `saveChat` 不吃防连点门——对话列宽是**拖拽**驱动的（ConversationWidthHandles
//     在 pointerup 里发最后一档宽度），门会把"最后一次"当连点丢掉 = 宽度回不去。
import { defineStore } from "pinia";
import { getSettings, llmProviderDelete, llmProviderSave, setSettings } from "../lib/api";
import { applyLocalePref } from "../i18n";
import { applyThemePref } from "../lib/theme";
import type {
  ChatUpdate,
  ContextUpdate,
  DeveloperUpdate,
  EmbeddingUpdate,
  LlmUpdate,
  MemoryUpdate,
  PluginsUpdate,
  ProviderInput,
  SettingsView,
  
  ToolsUpdate,
} from "../types";

/** `setSettings` 的入参形状从这里取，不在第二处重列那十二个键（键清单的真源 = `lib/api.ts`）。 */
type SettingsPatch = Parameters<typeof setSettings>[0];

/** 「已保存」提示的驻留时长（§13.5 观感定档，十个动作共用一把尺）。 */
const SAVED_FLASH_MS = 2000;

export const useSettingsStore = defineStore("settings", {
  state: () => ({
    view: null as SettingsView | null,
    /** LLM 表单保存中（按钮禁用、防连点）。 */
    saving: false,
    /** 最近一次保存刚成功（短暂显示“已保存”）。 */
    savedFlash: false,
    /** 最近一次失败提示（组件红条显示，下一动作清除）。 */
    error: "",
    /** `savedFlash` 的**唯一**计时器句柄。审计 C13 登记的同源小缺陷 = 旧形态每份
     *  副本各自 `setTimeout(() => (this.savedFlash = false), 2000)` 而不留句柄，
     *  于是连续两次保存时**前一次的 timer 会把后一次刚亮的提示提前关掉**（亮 0.5s
     *  就灭）。现在复用句柄：新的 `markSaved()` 先 `clearTimeout` 旧的再重挂。 */
    flashTimer: undefined as ReturnType<typeof setTimeout> | undefined,
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
    /** 重拉投影（v2.58 档位命令后刷新 llm 区视图——llm_provider_save/
     * delete 有自己的返回值，但 SettingsView 是唯一前端真源）。 */
    async refresh() {
      try {
        this.view = await getSettings();
      } catch (e) {
        this.error = String(e);
      }
    },
    /** 档位增改（v2.58 档位制）：命令返回全量档位投影，这里以重拉投影
     * 收口（活动档平面速记一并刷新）。失败进 error 由调用方显示。 */
    async saveProvider(input: ProviderInput) {
      this.error = "";
      try {
        await llmProviderSave(input);
        await this.refresh();
        return true;
      } catch (e) {
        this.error = String(e);
        return false;
      }
    },
    /** 档位删除（删活动档 = 指针落首档）。 */
    async deleteProvider(id: string) {
      this.error = "";
      try {
        await llmProviderDelete(id);
        await this.refresh();
        return true;
      } catch (e) {
        this.error = String(e);
        return false;
      }
    },
    /** 点亮「已保存」并**复用同一个计时器句柄**（见 state.flashTimer 的理由）。 */
    markSaved() {
      this.savedFlash = true;
      if (this.flashTimer !== undefined) clearTimeout(this.flashTimer);
      this.flashTimer = setTimeout(() => {
        this.savedFlash = false;
        this.flashTimer = undefined;
      }, SAVED_FLASH_MS);
    },
    /**
     * 保存类动作的**唯一**实现：防连点门 + 清 error + 部分更新 + 投影回写 +
     * 短暂「已保存」+ 失败进 error。`guard = false` 只给拖拽收尾那一个例外
     * （`saveChat`），`flash = false` 只给 `saveThinking`。
     *
     * ⚠ `flash = false` 时**连 `savedFlash` 都不碰**（不只是"成功后不点亮"）：
     * 旧 `saveThinking` 就没有那句 `this.savedFlash = false`，于"上一次保存的提示
     * 还亮着时切思考语言"这一路径给的是"继续亮到点"。等价判据把这条当成一个
     * 独立用例盯着（L3 的 `flashOffTiming`）。
     */
    async patchSettings(
      patch: SettingsPatch,
      opts: { guard?: boolean; flash?: boolean } = {},
    ) {
      const { guard = true, flash = true } = opts;
      if (guard && this.saving) return;
      this.error = "";
      if (guard) this.saving = true;
      if (flash) this.savedFlash = false;
      try {
        this.view = await setSettings(patch);
        if (flash) this.markSaved();
      } catch (e) {
        this.error = String(e);
      } finally {
        if (guard) this.saving = false;
      }
    },
    /** 保存 LLM 配置；空串字段语义 = 清除（后端 settings.rs）。 */
    async saveLlm(llm: LlmUpdate) {
      await this.patchSettings({ llm });
    },
    /** 保存全局快捷键（tray 即时重注册；键位被占用时后端报错）。 */
    async saveShortcut(shortcut: string) {
      await this.patchSettings({ shortcut });
    },
    /** 保存上下文管理区（⑨）：后端校验 + 换入 ctx-compact 活配置，
     * 保存即生效。校验失败（阈值出界等）在 error 条显示。 */
    async saveContext(context: ContextUpdate) {
      await this.patchSettings({ context });
    },
    /** 保存记忆注入区（⑪）：后端校验 + 换入 Memory-Inject 活配置，
     * 保存即生效（下一轮请求就按新条数注入）。 */
    async saveMemory(memory: MemoryUpdate) {
      await this.patchSettings({ memory });
    },
    /** 保存工具暴露区（⑫）：后端校验 + 换入 ToolExposure 活配置，
     * 保存即生效（下一轮请求按新上限截断）。 */
    async saveTools(tools: ToolsUpdate) {
      await this.patchSettings({ tools });
    },
    /** 保存开发者选项区（v2.49）：解锁态 + 危险开关。解锁 = 隐藏分区
     * 三处（导航轨/⌘K/面板）立即可见；危险开关保存即被后端命令直读。 */
    async saveDeveloper(developer: DeveloperUpdate) {
      await this.patchSettings({ developer });
    },
    /** 保存思考语言（docs/design/NEW/language of thought.md）：落盘即
     * 成功——当前会话的 system 前缀已锁死不受影响，「下个会话生效」
     * 的提示由调用方（LlmSection）负责 ⇒ 这里**不闪**「已保存」。 */
    async saveThinking(thinking: { language: string }) {
      await this.patchSettings({ thinking }, { flash: false });
    },
    /** 保存 embedding 区（⑬）：后端热重建共享客户端，检索/经历/提取
     * 随即生效。空串字段语义 = 清除。 */
    async saveEmbedding(embedding: EmbeddingUpdate) {
      await this.patchSettings({ embedding });
    },
    /** 保存插件管理区（⑬）：自动停用天数（0 = 关）。天数 > 0 时后端
     * 保存即巡检一遍；关闭不复活已停用插件。 */
    async savePlugins(plugins: PluginsUpdate) {
      await this.patchSettings({ plugins });
    },
    /** 保存对话行为区（UI shell 文档 §3.2 ChatSection / §2.7 列宽偏好）：
     * 列宽 / 默认展开过程 / 回车发送 / 用量读数口径，保存即生效。
     * ⚠ `guard = false` 是**有意的例外**（不是漏写）：列宽由拖拽收尾发最后一档
     * 宽度（ConversationWidthHandles 的 pointerup），防连点门会把"最后一次"当
     * 连点丢掉 = 松手后的宽度回不去。旧形态这段本就没有门，此处按原语义保留。 */
    async saveChat(chat: ChatUpdate) {
      await this.patchSettings({ chat }, { guard: false });
    },
  },
});
