// 上下文管理（⑨，Context and Memory.md §3）前端侧的少量纯函数。
// 阈值换算的事实源在后端（SettingsView.context 已带生效值）；这里只补
// 表单编辑中的即时建议值展示（与 kernel_agent::suggested_threshold 同表）。

/** 动态建议阈值：窗口越大建议越低（§3.2.3）。 */
export function suggestedThreshold(windowTokens: number): number {
  if (windowTokens <= 16_000) return 30; // 本地小模型
  if (windowTokens <= 64_000) return 35; // 中等窗口
  if (windowTokens <= 256_000) return 25; // 大窗口
  return 15; // 1M 及以上
}

/** 窗口未设置时后端的保守回落（settings.rs FALLBACK_WINDOW_TOKENS）。 */
export const FALLBACK_WINDOW_TOKENS = 32_000;
