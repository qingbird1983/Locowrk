/** **开发者分区的即时读数 + 记忆树手动整理**（⑫ / M6.1 / M2 §3.36③）：工具暴露截断
 * 快照、前缀断裂诊断、记忆树影子运行观测——三条读数都是"打开分区拉一次"，不推送不轮询。
 *
 * ⚠ 原先这条 `organizeNow` 也寄居在本片，但它的消费者是 chat store 的发送链（`/整理`）
 * 而不是开发者面板 ⇒ 那片头注当时留了一句"挪它 = 语义改动，另刀另证"。S2 刀⑦ 就是那
 * 另刀：`/整理` 改成了 `/压缩`（压缩的人手侧住 `context.ts`），而**手动整理本身不能删**
 * ——它是自动整理熔断（`paused_auto`）之后唯一的复位入口（`memory-tree/tree.rs` 的
 * `mark_manual` 只被这条链吃）。所以它现在回到本区、由记忆树观测面板那颗按钮发起，
 * 名与实终于对上：开发者面里的影子运行操作。 */

import { invoke } from "@tauri-apps/api/core";
import type { CacheBreakReport, ExposureDebug, TreeDebug } from "../../types";
/** 最近一次工具暴露截断快照（⑫ 开发者面板；打开分区时拉取）。 */
export function toolExposureDebug(): Promise<ExposureDebug> {
  return invoke("tool_exposure_debug");
}

/** 手动整理（M2 · 开发者面板那颗按钮）：立即整理当前未整理尾巴并**复位自动整理熔断**。
 * 发起即返回（本体后台跑），收束靠 `lw://organize-done` 推送。 */
export function organizeNow(sessionId: string): Promise<void> {
  return invoke("organize_now", { sessionId });
}

/** 前缀断裂诊断（M6.1；会话最近一步 hash 链比对，开关关 = null）。 */
export function llmCacheDiagnose(sessionId: string): Promise<CacheBreakReport | null> {
  return invoke("llm_cache_diagnose", { sessionId });
}

/** 记忆树影子运行观测（M2 §3.36③；开发者面板打开时拉取）。 */
export function memoryTreeView(sessionId: string): Promise<TreeDebug> {
  return invoke("memory_tree_view", { sessionId });
}
