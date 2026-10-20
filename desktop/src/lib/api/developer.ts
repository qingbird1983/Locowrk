/** **开发者分区的即时读数**（⑫ / M6.1）：工具暴露截断快照与前缀断裂诊断——
 * 两条读数都是"打开分区拉一次"，不推送不轮询。
 *
 * S4 刀③b：`organizeNow` 与 `memoryTreeView` 随 organize 宿主模块整面删除
 * （命令本体与前端包装同刀；`/整理` 已在 S2 刀⑦ 改成 `/压缩`）。 */

import { invoke } from "@tauri-apps/api/core";
import type { CacheBreakReport, ExposureDebug } from "../../types";
/** 最近一次工具暴露截断快照（⑫ 开发者面板；打开分区时拉取）。 */
export function toolExposureDebug(): Promise<ExposureDebug> {
  return invoke("tool_exposure_debug");
}

/** 前缀断裂诊断（M6.1；会话最近一步 hash 链比对，开关关 = null）。 */
export function llmCacheDiagnose(sessionId: string): Promise<CacheBreakReport | null> {
  return invoke("llm_cache_diagnose", { sessionId });
}
