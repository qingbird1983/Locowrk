/** **开发者分区的即时读数**（⑫ / M6.1 / M2 §3.36③）：工具暴露截断快照、前缀断裂诊断、
 * 记忆树影子运行观测——三条都是"打开分区拉一次"，不推送不轮询。
 *
 * ⚠ 中间那条手动整理触发寄居在本片：它的消费者是 chat store 的发送链而不是开发者面板，
 * 但原文把四条排在一起 ⇒ 保序搬运不许重排（挪它 = 语义改动，另刀另证）。 */

import { invoke } from "@tauri-apps/api/core";
import type { CacheBreakReport, ExposureDebug, TreeDebug } from "../../types";
/** 最近一次工具暴露截断快照（⑫ 开发者面板；打开分区时拉取）。 */
export function toolExposureDebug(): Promise<ExposureDebug> {
  return invoke("tool_exposure_debug");
}

/** 手动整理（M2 `/整理`）：立即整理当前未整理尾巴（发起即返回，后台跑）。 */
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
