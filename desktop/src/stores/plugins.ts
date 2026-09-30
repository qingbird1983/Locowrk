// 插件面板状态（§13.3 / §13.4）：双页签数据 + 操作动作。
// 事件驱动的实时同步策略从简：任何 state-changed 都整表重拉——
// 本地目录市场与 lockfile 都是毫秒级本地 IO，不值得做增量补丁。
import { defineStore } from "pinia";
import {
  pluginBatchSetEnabled,
  pluginBatchUninstall,
  pluginInstall,
  pluginList,
  pluginListWithUsage,
  pluginRegistryList,
  pluginRollback,
  pluginSetEnabled,
  pluginUninstall,
  pluginUpdate,
  subscribePluginState,
} from "../lib/api";
import type { InstalledPlugin, MarketplaceItem, PluginUsageView } from "../types";

export const usePluginStore = defineStore("plugins", {
  state: () => ({
    installed: [] as InstalledPlugin[],
    marketplace: [] as MarketplaceItem[],
    /** ⑩⑤ 使用统计投影（plugin_list_with_usage），按 id join。 */
    usage: [] as PluginUsageView[],
    /** 正在执行操作的插件 id（按钮转圈、防连点）。 */
    busy: [] as string[],
    /** 最近一次操作失败的提示（组件里红条显示，下一个动作清除）。 */
    error: "",
  }),
  actions: {
    async init() {
      await this.refresh();
      await subscribePluginState(() => {
        void this.refresh();
      });
    },
    async refresh() {
      try {
        const [installed, marketplace, usage] = await Promise.all([
          pluginList(),
          pluginRegistryList(),
          pluginListWithUsage(),
        ]);
        this.installed = installed;
        this.marketplace = marketplace;
        this.usage = usage;
      } catch (e) {
        this.error = String(e);
      }
    },
    usageOf(id: string): PluginUsageView | undefined {
      return this.usage.find((u) => u.id === id);
    },
    /** ⑩⑥ 批量停用/启用。 */
    async batchSetEnabled(ids: string[], enabled: boolean) {
      this.error = "";
      const res = await pluginBatchSetEnabled(ids, enabled);
      await this.refresh();
      return res;
    },
    /** ⑩⑥ 批量卸载（调用方先二次确认列名，§9.5）。 */
    async batchUninstall(ids: string[]) {
      this.error = "";
      const res = await pluginBatchUninstall(ids);
      await this.refresh();
      return res;
    },
    async install(id: string, version?: string) {
      await this.run(id, async () => {
        await pluginInstall(id, version);
      });
    },
    async setEnabled(id: string, enabled: boolean) {
      await this.run(id, async () => {
        await pluginSetEnabled(id, enabled);
      });
    },
    async uninstall(id: string) {
      await this.run(id, async () => {
        await pluginUninstall(id);
      });
    },
    async rollback(id: string, version: string) {
      await this.run(id, async () => {
        await pluginRollback(id, version);
      });
    },
    async update(id: string) {
      await this.run(id, async () => {
        await pluginUpdate(id);
      });
    },
    isBusy(id: string): boolean {
      return this.busy.includes(id);
    },
    /** 统一的动作包装：busy 标记 + 错误收集。state-changed 事件会触发
     * 整表重拉，动作本身不必手动刷新。 */
    async run(id: string, op: () => Promise<unknown>) {
      if (this.isBusy(id)) return;
      this.error = "";
      this.busy.push(id);
      try {
        await op();
        // 等一拍再拉：等 state-changed 广播先行到达，避免竞态双拉
        await this.refresh();
      } catch (e) {
        this.error = String(e);
      } finally {
        this.busy = this.busy.filter((b) => b !== id);
      }
    },
  },
});
