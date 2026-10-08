/**
 * 模型能力目录前端辅助（批 A，供应商稿 §4）：目录命中 → 表格行合并、
 * 空行种子、价格/模态徽章格式化。纯函数 + 无请求——目录数据全部走
 * llm_lookup_model 命令（后端内存快照），这里只做形状搬运。
 */
import type {
  CatalogModelInfo,
  ProviderModelPricing,
  ProviderModelSources,
  ProviderModelView,
} from "../types";

/** 全未知来源（自定义模型未命中目录的种子形态：不再触发合并回环）。 */
export const EMPTY_SOURCES: ProviderModelSources = {
  window: null,
  maxOutput: null,
  modalities: null,
  reasoning: null,
};

/** 客户端新建行（sources = null = 待目录合并；后端投影恒带 sources）。 */
export function emptyModelRow(id: string): ProviderModelView {
  return {
    id,
    alias: null,
    contextWindow: null,
    maxOutput: null,
    maxOutputSource: null,
    supportsImage: null,
  availableForSubagents: null,
    inputModalities: [],
    reasoningLadder: [],
    toolCall: null,
    pricing: null,
    sources: null,
  };
}

/** 目录命中 → 行的目录派生面（钉死面不碰，id 原样保留）。 */
export function catalogHitToRow(
  base: ProviderModelView,
  hit: CatalogModelInfo,
): ProviderModelView {
  return {
    ...base,
    contextWindow: base.sources?.window === "user" ? base.contextWindow : hit.contextWindow,
    maxOutput: base.sources?.maxOutput === "user" ? base.maxOutput : hit.maxOutput,
    inputModalities:
      hit.inputModalities.length > 0 ? hit.inputModalities : base.inputModalities,
    reasoningLadder: hit.reasoningOptions,
    toolCall: hit.toolCall,
    pricing: hit.pricing,
    sources: {
      window:
        base.sources?.window ?? (hit.contextWindow != null ? "catalog" : null),
      maxOutput:
        base.sources?.maxOutput ?? (hit.maxOutput != null ? "catalog" : null),
      modalities:
        base.sources?.modalities ??
        (hit.inputModalities.length > 0 ? "catalog" : null),
      reasoning: hit.reasoningOptions.length > 0 ? "catalog" : null,
    },
  };
}

/** 目录未命中：落定全未知形态（防合并回环）。 */
export function markRowUnknown(base: ProviderModelView): ProviderModelView {
  return { ...base, sources: base.sources ?? EMPTY_SOURCES };
}

/** USD 单价短格式（每百万 token；null/undefined = 空串不渲染）。 */
export function formatPriceUsd(v: number | null | undefined): string {
  if (v == null) return "";
  // 目录价 0–4 位数：0.003 → "$0.003"，0.6 → "$0.6"，15 → "$15"
  const s = v >= 1 ? String(Math.round(v * 100) / 100) : String(v);
  return `$${s}`;
}

/** 价格行（in/out 双价齐备才渲染一行；USD 原值如实）。 */
export function formatPricingShort(p: ProviderModelPricing | null): string {
  if (!p) return "";
  const parts: string[] = [];
  const i = formatPriceUsd(p.input);
  const o = formatPriceUsd(p.output);
  if (i) parts.push(`in ${i}`);
  if (o) parts.push(`out ${o}`);
  return parts.join(" · ");
}

/** 展示模态含图像（未知模态 = 纯文本，不含）。 */
export function hasImageInput(row: {
  supportsImage: boolean | null;
  inputModalities: string[];
}): boolean {
  if (row.supportsImage != null) return row.supportsImage;
  return row.inputModalities.includes("image");
}
