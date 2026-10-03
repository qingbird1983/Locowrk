/**
 * 常见模型上下文窗口静态档案（S2，清单 §5"自动提取来源"之二）：云端
 * provider 不暴露窗口元数据，模型名命中前缀时自动填充"未设置"的窗口。
 * Ollama 走 /api/show 实测（更准），档案表只是兜底之一——手动选择永远
 * 优先，命中不覆盖用户已设的值。数字用 token 原值（档位呈现交给
 * compactNumber）。
 */
export const MODEL_WINDOW_PROFILES: ReadonlyArray<{
  match: string;
  window: number;
}> = [
  { match: "deepseek-chat", window: 131_072 },
  { match: "deepseek-reasoner", window: 131_072 },
  { match: "deepseek-flash", window: 1_048_576 },
  { match: "deepseek-v4", window: 1_048_576 },
  { match: "claude-", window: 200_000 },
  { match: "gpt-4o", window: 128_000 },
  { match: "gpt-4.1", window: 1_047_576 },
  { match: "gemini-2.5", window: 1_048_576 },
  { match: "gemini-2.0", window: 1_048_576 },
  { match: "llama3.1", window: 131_072 },
  { match: "qwen2.5", window: 131_072 },
  { match: "qwen3", window: 262_144 },
];

/** 模型名前缀命中 → 窗口 token；未命中返回 null（保持未设置）。 */
export function profileWindow(model: string): number | null {
  const name = model.trim().toLowerCase();
  if (!name) return null;
  const hit = MODEL_WINDOW_PROFILES.find((p) => name.startsWith(p.match));
  return hit ? hit.window : null;
}
