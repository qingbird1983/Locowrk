/** **端点能力探测**（S2 清单 §5 / ⑬）：窗口大小与向量维度两条。合一片的依据是它们
 * 是同一个动作的形状——**拿一条测试请求去问端点"你能装多少 / 你出几维"**，失败就报错给 UI 提示手动填。
 *
 * 窗口那条是**预留·零消费者**（原注释的 v2.137 四层链凭证跟着搬）。 */

import { invoke } from "@tauri-apps/api/core";
// ---- 模型窗口探测（S2，清单 §5）：Ollama /api/show 自动提取 ----
/** 探测 Ollama 模型上下文窗口（token）；失败 reject（UI 提示手动选择）。
 *
 * **预留·零消费者**（审计 C3 / 用户 2026-10-10 裁定保留）：窗口面自 v2.137
 * 四层链（会话覆盖 ?? 模型行钉死 ?? 目录 ?? 档位）起不再需要手动触发，
 * 设置页的「自动检测」钮随该批下线，本包装在 `api.ts` 之外无 import 方；
 * 后端 `llm/probe.rs` 因此同属零消费者面。退役需与命令 `llm_probe_window`
 * 同批（三向闭合同上）。 */
export function llmProbeWindow(baseUrl: string, model: string): Promise<number> {
  return invoke("llm_probe_window", { baseUrl: baseUrl || null, model });
}

// ---- embedding 探测（⑬）：嵌一条测试文本返回向量维度 ----

/** 探测 embedding 端点（不落盘）；失败 reject（UI 提示）。 */
export function llmProbeEmbedding(
  baseUrl: string | null,
  model: string,
  apiKey?: string,
): Promise<number> {
  return invoke("llm_probe_embedding", {
    baseUrl: baseUrl || null,
    model,
    apiKey: apiKey || null,
  });
}
