/** **模型面**（批 A / 供应商稿 §4.5 / §8.2 §8.3 / v2.58 档位制）：档位模型清单、能力目录
 * 状态与在线刷新、单模型查询、探测与导入、逐模型与表单连通测试、网络代理、档位增删排。
 *
 * 本片的"全部档位的模型清单"与"窗口探测"两条是**预留·零消费者**（原注释的裁定凭证跟着搬，
 * 退役需与后端命令同批）。那条"打开外部网页链接"寄居在本片：它服务的是这一族的
 * 「如何获取 API Key」出口，不认模型语义，但原文把它排在查询与探测之间 ⇒ 同样按保序留在本族。 */

import { invoke } from "@tauri-apps/api/core";
import type {
  CatalogModelInfo,
  CatalogStatus,
  ImportApplyReport,
  ImportScanOutcome,
  LlmSettingsView,
  ModelTestResult,
  ProbeOutcome,
  ProviderInput,
  ProviderModels,
  ProviderView,
} from "../../types";
/** 全部档位的模型清单（§3.30 选择器数据源）：并行探测逐组收口，
 * 单档失败进 error 不拖垮其它组。
 *
 * **预留·零消费者**（审计 C3 / 用户 2026-10-10 裁定保留）：模型清单面现走
 * 目录快照 + 档位内 `llm_probe_models`，本包装在 `api.ts` 之外无任何 import
 * 方（审计逐条复核过，非目测）。退役需与后端命令 `llm_list_models` 同批，
 * 否则 `check-commands.mjs` 三向闭合当场判红。 */
export function llmListModels(): Promise<ProviderModels[]> {
  return invoke("llm_list_models");
}

/** 模型能力目录状态（批 A）：快照 bundled / 在线刷新 refreshed；首访
 * 惰性加载随包快照。 */
export function llmCatalogStatus(): Promise<CatalogStatus> {
  return invoke("llm_catalog_status");
}

/** 在线刷新 models.dev 目录（只进内存，永不落盘；失败抛 Err 旧数据保留）。 */
export function llmCatalogRefresh(): Promise<CatalogStatus> {
  return invoke("llm_catalog_refresh");
}

/** 单模型目录查询（供应商稿 §4.5）：供应商定位 + 归一化 id 精确命中；
 * null = 能力未知（不借池不猜）。 */
export function llmLookupModel(
  kind: string,
  baseUrl: string | null,
  model: string,
): Promise<CatalogModelInfo | null> {
  return invoke("llm_lookup_model", { kind, baseUrl, model });
}

/** 打开外部网页链接（批 A「如何获取 API Key ↗」）；后端只放行 https。 */
export function openExternalUrl(url: string): Promise<void> {
  return invoke("open_external_url", { url });
}

/** 单档模型探测（设置表单未保存值；apiKey 空缺回退档位已存 Key——
 * 凭据库双读）。失败时端点扫描（§8.2），胜出候选随行回报。 */
export function llmProbeModels(
  kind: string,
  baseUrl: string | null,
  apiKey: string | null,
  profileId: string | null,
): Promise<ProbeOutcome> {
  return invoke("llm_probe_models", { kind, baseUrl, apiKey, profileId });
}

/** 导入面板扫描（批 F）：四源纯读预览；候选不带 Key（hasKey 布尔）。 */
export function llmImportScan(): Promise<ImportScanOutcome> {
  return invoke("llm_import_scan");
}

/** 导入选中项：新档追加落盘；返回逐条结果（导入/跳过带因）。 */
export function llmImportApply(ids: string[]): Promise<ImportApplyReport> {
  return invoke("llm_import_apply", { ids });
}

/** 逐模型连通测试（§8.3）：最小请求 ping，五分类错误 + 延迟如实。 */
export function llmTestModel(profileId: string, model: string): Promise<ModelTestResult> {
  return invoke("llm_test_model", { profileId, model });
}

/** 表单连通测试（v2.143）：编辑模态「测试连接」吃表单当前值（未保存也
 * 可测）；Key 留空回落 profileId 已存 Key（后端同 llm_probe_models 口径）。 */
export function llmTestCustom(
  kind: string,
  baseUrl: string | null,
  apiKey: string | null,
  profileId: string | null,
  model: string,
): Promise<ModelTestResult> {
  return invoke("llm_test_custom", { kind, baseUrl, apiKey, profileId, model });
}

/** 网络代理设置（§8.5）：system / direct / custom（url）；保存即热换。
 * 返回更新后的 llm 区视图（目录合成随行）。 */
export function llmSetProxy(
  mode: "system" | "direct" | "custom",
  url: string | null,
): Promise<LlmSettingsView> {
  return invoke("llm_set_proxy", { mode, url });
}

/** 档位增改（v2.58 档位制）：id 缺省 = 新建；返回全量档位投影。 */
export function llmProviderSave(input: ProviderInput): Promise<ProviderView[]> {
  return invoke("llm_provider_save", { input });
}

/** 档位删除：删活动档 = 指针落首档；返回全量档位投影。 */
export function llmProviderDelete(id: string): Promise<ProviderView[]> {
  return invoke("llm_provider_delete", { id });
}

/** 档位排序（§3.30 服务列表拖拽落盘）：按 id 顺序整表重排。 */
export function llmProviderReorder(ids: string[]): Promise<ProviderView[]> {
  return invoke("llm_provider_reorder", { ids });
}
